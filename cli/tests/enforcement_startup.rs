mod scratch_folder;
mod settings_builder;
use leko_cli::enforcement::enforcement_loop::run;
use scratch_folder::Scratch;
use settings_builder::write_settings;

#[test]
fn invalid_startup_inputs_stop_enforcement() {
    let folder = Scratch::new("enforcement-startup");
    let file = write_settings(folder.path(), "settings.zip", &[]);
    for (path, zone) in [("/no/such/leko-settings.zip", Some("UTC".to_string())), (file.to_str().unwrap(), None)] {
        let mut err = Vec::new();
        let code = run(path, || panic!("clock must not be read"), || zone.clone(), || panic!("apps must not be read"), |_| panic!("nothing may terminate"), 99, || 0, |_| {}, |_| Ok(()), &mut err);
        assert_eq!(code, 1);
        assert!(!err.is_empty());
    }
}
