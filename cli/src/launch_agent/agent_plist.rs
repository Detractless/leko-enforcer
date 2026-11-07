//! Encodes the exact, deliberately limited launchd configuration owned by Leko.
use std::path::Path;

pub const LABEL: &str = "org.leko.enforcer.agent";
pub const FILE_NAME: &str = "org.leko.enforcer.agent.plist";

fn xml(value: &str) -> Result<String, String> {
    if value.chars().any(|c| (c < '\u{20}' && !matches!(c, '\t' | '\n' | '\r')) || matches!(c, '\u{fffe}' | '\u{ffff}')) {
        return Err("A path contains a character that cannot be written to a plist.".into());
    }
    Ok(value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;").replace('\r', "&#13;"))
}

pub fn render(label: &str, executable: &Path, settings: &Path, log: &Path) -> Result<String, String> {
    let path = |value: &Path| -> Result<String, String> {
        if !value.is_absolute() { return Err("Paths must be absolute.".into()); }
        xml(value.to_str().ok_or("Paths must be valid UTF-8.")?)
    };
    let (label, executable, settings, log) = (xml(label)?, path(executable)?, path(settings)?, path(log)?);
    Ok(format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict>\n<key>Label</key><string>{label}</string>\n<key>ProgramArguments</key><array><string>{executable}</string><string>enforce</string><string>{settings}</string></array>\n<key>RunAtLoad</key><true/>\n<key>KeepAlive</key><true/>\n<key>ThrottleInterval</key><integer>30</integer>\n<key>StandardErrorPath</key><string>{log}</string>\n</dict></plist>\n"))
}

/// Accept only the exact plist format this program writes, including its log path and label.
pub fn owned(contents: &str, log: &Path) -> bool {
    let prefix = "<key>ProgramArguments</key><array><string>";
    let Some((_, rest)) = contents.split_once(prefix) else { return false; };
    let Some((exe, rest)) = rest.split_once("</string><string>enforce</string><string>") else { return false; };
    let Some((settings, _)) = rest.split_once("</string></array>") else { return false; };
    let decode = |text: &str| text.replace("&#13;", "\r").replace("&apos;", "'").replace("&quot;", "\"").replace("&gt;", ">").replace("&lt;", "<").replace("&amp;", "&");
    render(LABEL, Path::new(&decode(exe)), Path::new(&decode(settings)), log).is_ok_and(|rendered| rendered == contents)
}
