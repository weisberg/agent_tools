use assert_cmd::Command;

pub fn bin() -> Command {
    Command::cargo_bin("mdli").expect("mdli binary")
}
