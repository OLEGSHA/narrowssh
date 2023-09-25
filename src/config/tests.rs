pub use std::path::PathBuf;

pub use crate::workspace::mock::MockWorkspace;

pub use super::*;

/// Tests for [`visit_config_files`]
mod visit_config_files {
    use super::*;

    /// Invokes [`visit_config_files`] and checks the visited paths.
    ///
    /// Order of visited paths is important.
    fn must_visit<'a, P, W, I>(
        file: P,
        owner: uid_t,
        ws: &W,
        mut paths: I,
    ) -> Result<()>
    where
        P: AsRef<Path>,
        W: Workspace,
        I: Iterator<Item = &'a PathBuf>,
    {
        visit_config_files(
            file,
            owner,
            |p| {
                assert_eq!(
                    paths.next().map(|x| x.canonicalize().unwrap()),
                    Some(p.canonicalize().unwrap())
                );
                Ok(())
            },
            ws,
        )?;
        assert_eq!(paths.next(), None);

        Ok(())
    }

    /// Invokes [`visit_config_files`] and ensures it returns an [`Err`].
    fn must_fail<P, W>(file: P, owner: uid_t, ws: &W) -> Result<()>
    where
        P: AsRef<Path>,
        W: Workspace,
    {
        assert!(visit_config_files(file, owner, |_| Ok(()), ws).is_err());
        Ok(())
    }

    //
    // Basics
    //

    #[test]
    fn main_file_only() -> Result<()> {
        let mut ws = MockWorkspace::new()?;

        ws.add_user(1234, "alice", "home/alice")?;
        let main =
            ws.add_file("etc/main.conf", 1234, 0o600, "I am contents")?;

        must_visit(&main, 1234, &ws, [&main].into_iter())
    }

    #[test]
    fn main_file_and_empty_dir() -> Result<()> {
        let mut ws = MockWorkspace::new()?;

        ws.add_user(1234, "alice", "home/alice")?;
        let main =
            ws.add_file("etc/main.conf", 1234, 0o600, "I am contents")?;
        ws.add_dir("etc/main.conf.d/", 1234, 0o700)?;

        must_visit(&main, 1234, &ws, [&main].into_iter())
    }

    #[test]
    fn main_file_and_extension() -> Result<()> {
        let mut ws = MockWorkspace::new()?;

        ws.add_user(1234, "alice", "home/alice")?;
        let main =
            ws.add_file("etc/main.conf", 1234, 0o600, "I am contents")?;
        ws.add_dir("etc/main.conf.d/", 1234, 0o700)?;
        let xt =
            ws.add_file("etc/main.conf.d/xtra.conf", 1234, 0o600, "X")?;

        must_visit(&main, 1234, &ws, [&main, &xt].into_iter())
    }

    #[test]
    fn extension_order() -> Result<()> {
        let mut ws = MockWorkspace::new()?;

        ws.add_user(1234, "alice", "home/alice")?;
        let main =
            ws.add_file("etc/main.conf", 1234, 0o600, "I am contents")?;
        ws.add_dir("etc/main.conf.d/", 1234, 0o700)?;

        let mut add_ext = |s| {
            ws.add_file(
                format!("etc/main.conf.d/{}.conf", s),
                1234,
                0o600,
                "X",
            )
        };

        let x1 = add_ext("02.a")?;
        let x2 = add_ext("02-a")?;
        let x3 = add_ext("10")?;
        let x4 = add_ext("02.conf")?;
        let x5 = add_ext("weird")?;
        let x6 = add_ext("02")?;
        let x7 = add_ext("12")?;
        let x8 = add_ext("02~a")?;
        let x9 = add_ext("01")?;

        must_visit(
            &main,
            1234,
            &ws,
            [&main, &x9, &x6, &x2, &x1, &x4, &x8, &x3, &x7, &x5].into_iter(),
        )
    }

    #[test]
    fn ignore_unrelated_files() -> Result<()> {
        let mut ws = MockWorkspace::new()?;

        ws.add_user(1234, "alice", "home/alice")?;
        let main =
            ws.add_file("etc/main.conf", 1234, 0o600, "I am contents")?;
        ws.add_dir("etc/main.conf.d/", 1234, 0o700)?;

        let xt =
            ws.add_file("etc/main.conf.d/visit_me.conf", 1234, 0o600, "X")?;
        ws.add_file("etc/main.conf.d/no_file_ext", 1234, 0o600, "X")?;
        ws.add_dir("etc/main.conf.d/some_dir", 1234, 0o700)?;
        ws.add_file(
            "etc/main.conf.d/some_dir/grandchild.conf",
            1234,
            0o600,
            "X",
        )?;
        ws.add_file("etc/main.conf.d/wrong_file_ext.txt", 1234, 0o600, "X")?;
        ws.add_file("etc/main.d/wrong_dir.conf", 1234, 0o600, "X")?;

        must_visit(&main, 1234, &ws, [&main, &xt].into_iter())
    }

    #[test]
    fn ignore_errors_in_ignored_files() -> Result<()> {
        let mut ws = MockWorkspace::new()?;

        ws.add_user(1234, "alice", "home/alice")?;
        ws.add_user(5678, "mallory", "home/mallory")?;
        let main =
            ws.add_file("etc/main.conf", 1234, 0o600, "I am contents")?;
        ws.add_dir("etc/main.conf.d/", 1234, 0o700)?;

        let xt =
            ws.add_file("etc/main.conf.d/visit_me.conf", 1234, 0o600, "X")?;
        ws.add_file("etc/main.conf.d/wrong_owner.txt", 5678, 0o600, "X")?;
        ws.add_file("etc/main.conf.d/bad_perms.txt", 1234, 0o755, "X")?;
        ws.add_symlink(
            "etc/main.conf.d/broken_symlink.txt",
            "does/not/exist",
        )?;

        must_visit(&main, 1234, &ws, [&main, &xt].into_iter())
    }

    // Main file
    mod main {
        use super::*;

        #[test]
        fn missing() -> Result<()> {
            let ws = MockWorkspace::new()?;
            let main = ws.path("does/not/exist");
            must_fail(&main, 1234, &ws)
        }

        #[test]
        fn unreadable() -> Result<()> {
            let ws = MockWorkspace::new()?;
            let main = ws.add_symlink("etc/main.conf", "does/not/exist")?;
            must_fail(&main, 1234, &ws)
        }

        #[test]
        fn symlink() -> Result<()> {
            let mut ws = MockWorkspace::new()?;

            ws.add_user(1234, "alice", "home/alice")?;
            ws.add_file("etc/real.conf", 1234, 0o600, "I am contents")?;
            let main = ws.add_symlink("etc/main.conf", "etc/real.conf")?;

            must_visit(&main, 1234, &ws, [&main].into_iter())
        }

        #[test]
        fn hijacked() -> Result<()> {
            let mut ws = MockWorkspace::new()?;

            ws.add_user(1234, "alice", "home/alice")?;
            ws.add_user(5678, "mallory", "home/mallory")?;

            ws.add_file("etc/evil.conf", 5678, 0o600, "I am contents")?;
            let main = ws.add_symlink("etc/main.conf", "etc/evil.conf")?;

            must_fail(&main, 1234, &ws)
        }

        #[test]
        fn insecure() -> Result<()> {
            for owner in [1234, 5678] {
                for mode in [0o601, 0o610, 0o644, 0o701, 0o710, 0o755] {
                    let mut ws = MockWorkspace::new()?;

                    ws.add_user(1234, "alice", "home/alice")?;
                    ws.add_user(5678, "mallory", "home/mallory")?;
                    let main =
                        ws.add_file("etc/main.conf", owner, mode, "M")?;
                    must_fail(&main, 1234, &ws)?;
                }
            }

            Ok(())
        }
    }

    // Extensions directory
    mod dir {
        use super::*;

        #[test]
        fn unreadable() -> Result<()> {
            let mut ws = MockWorkspace::new()?;

            ws.add_user(1234, "alice", "home/alice")?;
            let main =
                ws.add_file("etc/main.conf", 1234, 0o600, "I am contents")?;
            ws.add_dir("etc/main.conf.d", 1234, 0o000)?;

            must_fail(&main, 1234, &ws)
        }

        #[test]
        fn symlink() -> Result<()> {
            let mut ws = MockWorkspace::new()?;

            ws.add_user(1234, "alice", "home/alice")?;
            let main =
                ws.add_file("etc/main.conf", 1234, 0o600, "I am contents")?;
            ws.add_dir("etc/real/", 1234, 0o700)?;
            let xt = ws.add_file("etc/real/xt.conf", 1234, 0o600, "X")?;
            ws.add_symlink("etc/main.conf.d", "etc/real")?;

            must_visit(&main, 1234, &ws, [&main, &xt].into_iter())
        }

        #[test]
        fn hijacked() -> Result<()> {
            let mut ws = MockWorkspace::new()?;

            ws.add_user(1234, "alice", "home/alice")?;
            ws.add_user(5678, "mallory", "home/mallory")?;

            let main =
                ws.add_file("etc/main.conf", 1234, 0o600, "I am contents")?;
            ws.add_dir("etc/real/", 5678, 0o700)?;
            ws.add_file("etc/real/xt.conf", 1234, 0o600, "X")?;
            ws.add_symlink("etc/main.conf.d", "etc/real")?;

            must_fail(&main, 1234, &ws)
        }

        #[test]
        fn insecure() -> Result<()> {
            for owner in [1234, 5678] {
                for mode in [0o701, 0o710, 0o755] {
                    let mut ws = MockWorkspace::new()?;

                    ws.add_user(1234, "alice", "home/alice")?;
                    ws.add_user(5678, "mallory", "home/mallory")?;

                    let main =
                        ws.add_file("etc/main.conf", 1234, 0o600, "M")?;
                    ws.add_dir("etc/main.conf.d/", owner, mode)?;
                    ws.add_file(
                        "etc/main.conf.d/xtra.conf",
                        1234,
                        0o600,
                        "X",
                    )?;

                    must_fail(&main, 1234, &ws)?;
                }
            }

            Ok(())
        }
    }

    // Extension file
    mod extensions {
        use super::*;

        #[test]
        fn unreadable() -> Result<()> {
            let mut ws = MockWorkspace::new()?;

            ws.add_user(1234, "alice", "home/alice")?;
            let main =
                ws.add_file("etc/main.conf", 1234, 0o600, "I am contents")?;
            ws.add_dir("etc/main.conf.d", 1234, 0o700)?;
            ws.add_symlink("etc/main.conf.d/xt.conf", "does/not/exist")?;

            must_fail(&main, 1234, &ws)
        }

        #[test]
        fn symlink() -> Result<()> {
            let mut ws = MockWorkspace::new()?;

            ws.add_user(1234, "alice", "home/alice")?;
            let main =
                ws.add_file("etc/main.conf", 1234, 0o600, "I am contents")?;
            ws.add_dir("etc/main.conf.d", 1234, 0o700)?;
            let xt = ws.add_file("etc/real/xt.conf", 1234, 0o600, "X")?;
            ws.add_symlink("etc/main.conf.d/xt.conf", "etc/real/xt.conf")?;

            must_visit(&main, 1234, &ws, [&main, &xt].into_iter())
        }

        #[test]
        fn hijacked() -> Result<()> {
            let mut ws = MockWorkspace::new()?;

            ws.add_user(1234, "alice", "home/alice")?;
            ws.add_user(5678, "mallory", "home/mallory")?;

            let main =
                ws.add_file("etc/main.conf", 1234, 0o600, "I am contents")?;
            ws.add_dir("etc/main.conf.d", 1234, 0o700)?;
            ws.add_file("etc/real/xt.conf", 5678, 0o600, "X")?;
            ws.add_symlink("etc/main.conf.d/xt.conf", "etc/real/xt.conf")?;

            must_fail(&main, 1234, &ws)
        }

        #[test]
        fn insecure() -> Result<()> {
            for owner in [1234, 5678] {
                for mode in [0o601, 0o610, 0o644, 0o701, 0o710, 0o755] {
                    let mut ws = MockWorkspace::new()?;

                    ws.add_user(1234, "alice", "home/alice")?;
                    ws.add_user(5678, "mallory", "home/mallory")?;

                    let main =
                        ws.add_file("etc/main.conf", 1234, 0o600, "M")?;
                    ws.add_dir("etc/main.conf.d/", 1234, 0o700)?;
                    ws.add_file(
                        "etc/main.conf.d/xtra.conf",
                        owner,
                        mode,
                        "X",
                    )?;

                    must_fail(&main, 1234, &ws)?;
                }
            }

            Ok(())
        }
    }
}

/// Tests for [`ControlManager::load`]
mod load_control {
    use super::*;

    fn load<S: AsRef<str>, const N: usize>(
        main: S,
        exts: [S; N],
    ) -> Result<ControlManager> {
        let mut ws = MockWorkspace::new()?;

        ws.add_user(0, "root", "root")?;
        ws.add_user(1, "daemon", "daemon-home")?;
        ws.add_user(1000, "alice", "home/alice")?;
        ws.add_user(1001, "bob", "home/bob")?;
        ws.add_user(1002, "charlie", "home/charlie")?;
        ws.add_user(1003, "dan", "home/dan")?;

        let main = ws.add_file("etc/main.toml", 0, 0o600, main.as_ref())?;
        ws.add_dir("etc/main.toml.d/", 0, 0o700)?;

        for (i, ext) in exts.into_iter().enumerate() {
            ws.add_file(
                format!("etc/main.toml.d/{:02}.toml", i),
                0,
                0o600,
                ext.as_ref(),
            )?;
        }

        ControlManager::load(&ws, main)
    }

    #[test]
    fn basic() -> Result<()> {
        let _cm = load(
            r#"
            # Generic example

            ["*"]
            enable = false
            config = "~/config.conf"
            authorized_keys = "~/.ssh/authorized_keys"

            [alice]
            enable = true

            [bob]
            enable = true
            config = "/etc/bobconfig.conf"
            authorized_keys = "/etc/bobauth"

            [charlie]
            config = "why/even/set/this"
        "#,
            [],
        )?;

        Ok(())
    }

    #[test]
    fn empty() -> Result<()> {
        let _cm = load("", [])?;
        Ok(())
    }

    #[test]
    fn invalid_toml() -> Result<()> {
        assert!(load("Not a valid TOML", []).is_err());
        Ok(())
    }
}
