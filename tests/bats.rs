macro_rules! bats_test {
    ($x:ident) => {
        #[test]
        fn $x() {
            assert!(std::process::Command::new("bats")
                .arg(format!("test/{}.bats", stringify!($x)))
                .env("CARGO_BIN_EXE_asdf", env!("CARGO_BIN_EXE_asdf"))
                .spawn()
                .unwrap()
                .wait()
                .unwrap()
                .success());
        }
    };
}

// Use `cargo build && bats test --filter <test case>` to run individual test cases

// Fails without fish
//bats_test!(asdf_fish);
bats_test!(asdf_sh);
// Not relevant in the rust codebase
//bats_test!(banned_commands);
bats_test!(current_command);
bats_test!(get_asdf_config_value);
bats_test!(help_command);
bats_test!(info_command);
bats_test!(install_command);
bats_test!(latest_command);
bats_test!(list_command);
bats_test!(plugin_add_command);
bats_test!(plugin_extension_command);
bats_test!(plugin_list_all_command);
bats_test!(plugin_remove_command);
bats_test!(plugin_test_command);
bats_test!(plugin_update_command);
bats_test!(remove_command);
bats_test!(reshim_command);
bats_test!(shim_env_command);
bats_test!(shim_exec);
bats_test!(shim_versions_command);
bats_test!(uninstall_command);
bats_test!(update_command);
// disabled because these are internal functions. See lib.rs
// bats_test!(utils);
bats_test!(version_commands);
bats_test!(where_command);
bats_test!(which_command);
