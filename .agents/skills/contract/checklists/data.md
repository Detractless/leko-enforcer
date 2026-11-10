# Data checklist
- DATA-1: Saving is atomic: a crash half-way leaves the old data readable, never a broken file.
- DATA-2: A missing, empty or corrupt data file is reported plainly and nothing is overwritten.
- DATA-3: Data written by an earlier version still loads, or the user is told how to convert it.
