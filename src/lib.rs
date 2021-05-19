use std::{path::Path, process::Command};

use quote::ToTokens;
use syn::{
    visit_mut::{visit_file_mut, visit_item_mod_mut, VisitMut},
    Attribute, File, Ident, Item, ItemMod,
};
use syn_inline_mod::parse_and_inline_modules;

pub struct Visitor;

impl VisitMut for Visitor {
    fn visit_file_mut(&mut self, file: &mut File) {
        file.items.retain(|item| Self::retain_item(item));
        visit_file_mut(self, file);
    }

    fn visit_item_mod_mut(&mut self, i: &mut ItemMod) {
        if let Some((_, items)) = &mut i.content {
            items.retain(|i| Self::retain_item(i))
        }
        visit_item_mod_mut(self, i);
    }
}

impl Visitor {
    fn has_test_attr(attrs: &Vec<Attribute>) -> bool {
        if attrs.len() > 0 {
            let cfg = attrs[0].path.get_ident();
            let attribute = attrs[0].parse_args::<Ident>();
            return match (cfg, attribute) {
                (Some(x), Ok(y)) if x.to_string() == "cfg" && y.to_string() == "test" => true,
                _ => false,
            };
        }
        return false;
    }

    fn retain_item(item: &Item) -> bool {
        match item {
            syn::Item::Mod(x) if x.attrs.len() > 0 => !Self::has_test_attr(&x.attrs),
            _ => true,
        }
    }
}

pub struct Bundle<P>
where
    P: AsRef<Path>,
{
    entry_module: P,
    output: P,
    strip_tests: bool,
    format_output: bool,
}

impl<P> Bundle<P>
where
    P: AsRef<Path>,
{
    pub fn new(entry_module: P, output: P) -> Self {
        Self {
            entry_module,
            output,
            strip_tests: false,
            format_output: true,
        }
    }

    pub fn stript_tests(mut self, value: bool) -> Self {
        self.strip_tests = value;
        self
    }

    pub fn build_output(self) -> std::io::Result<()> {
        let mut file = parse_and_inline_modules(self.entry_module.as_ref());
        if self.strip_tests {
            let mut v = Visitor {};
            v.visit_file_mut(&mut file);
        }

        std::fs::write(self.output.as_ref(), file.into_token_stream().to_string())?;

        if self.format_output {
            Command::new("rustfmt")
                .arg(self.output.as_ref())
                .spawn()?
                .wait()?;
        }

        Ok(())
    }
}

#[cfg(test_not_now)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    struct StubFileSystem<'a> {
        file_map: HashMap<&'a str, &'a [u8]>,
    }

    impl<'a> StubFileSystem<'a> {
        fn new() -> Self {
            Self {
                file_map: HashMap::new(),
            }
        }

        fn insert(&mut self, name: &'a str, content: &'a [u8]) {
            self.file_map.insert(name, content);
        }
    }

    impl<'a> FileSystem for StubFileSystem<'a> {
        type Reader = &'a [u8];

        fn open_submodule(
            &self,
            _relative_path: &str,
            module_name: &str,
        ) -> Result<Self::Reader, std::io::Error> {
            if let Some(m) = self.file_map.get(module_name) {
                Ok(m)
            } else {
                Err(std::io::Error::new(ErrorKind::NotFound, "File not found"))
            }
        }
    }

    fn prep_file_system() -> impl FileSystem {
        let mut map = StubFileSystem::new();
        map.insert(
            "main",
            r"use std::io;
use std::{BufReader};
pub mod game;
enum Test {
    One,
}"
            .as_bytes(),
        );
        map.insert(
            "game",
            r"struct Game {
    test: i32,
}
use std::fs::{File}
use std::io;"
                .as_bytes(),
        );
        map
    }

    fn line_from(line: &str, trim_left_count: usize, trim_right_count: usize) -> LineToken {
        syntax::LineToken::OtherLine {
            trimmed_ref: LineRef::new(
                trim_left_count,
                line.len() - trim_left_count - trim_right_count,
            ),
            line: line.to_string(),
        }
    }

    #[test]
    fn it_works() {
        let mut bundle = Bundle::new("main", prep_file_system());
        bundle.load().unwrap();
        assert_eq!(
            vec![
                LineToken::UseModule {
                    line: "use std::io;".to_string(),
                    name: LineRef::new(4, 7)
                },
                LineToken::UseManyModules {
                    names: vec![LineRef::new(10, 9)],
                    line: "use std::{BufReader};".to_string(),
                    parent: LineRef::new(4, 3)
                },
                LineToken::Module {
                    name: "game".to_string(),
                    is_pub: true,
                    tokens: vec![
                        line_from("struct Game {", 0, 0),
                        line_from("    test: i32,", 4, 0),
                        line_from("}", 0, 0),
                        line_from("use std::fs::{File}", 0, 0),
                        LineToken::UseModule {
                            line: "use std::io;".to_string(),
                            name: LineRef::new(4, 7)
                        }
                    ]
                },
                line_from("enum Test {", 0, 0),
                line_from("    One,", 4, 0),
                line_from("}", 0, 0)
            ],
            bundle.file
        );
    }

    #[test]
    fn write_works() {
        let files = prep_file_system();
        let mut bundle = Bundle::new("main", files);
        bundle.load().unwrap();
        let mut result = Vec::<u8>::new();
        bundle.write(&mut result).unwrap();
        assert_eq!(
            String::from_utf8(result).unwrap().as_str(),
            r"use std::io;
use std::{BufReader};
pub mod game{
struct Game {
    test: i32,
}
use std::fs::{File}
use std::io;
}
enum Test {
    One,
}
"
        )
    }

    #[test]
    fn test_bundle_with_files() {
        let temp_dir = tempdir::TempDir::new("bundle_rs").unwrap();
        std::fs::copy("./data/test-1/game.rs", temp_dir.path().join("game.rs")).unwrap();
        std::fs::copy("./data/test-1/main.rs", temp_dir.path().join("main.rs")).unwrap();
        let file_system = ModuleFileSystem::new(vec![temp_dir.path().to_str().unwrap()]);
        let mut bundle = Bundle::new("main", file_system);
        bundle.load().unwrap();
        bundle
            .write(&mut std::fs::File::create(temp_dir.path().join("expected.rs")).unwrap())
            .unwrap();
        println!("{}", temp_dir.path().to_str().unwrap());
        let left = std::fs::read_to_string("./data/test-1/expected.txt").unwrap();
        let right = std::fs::read_to_string(temp_dir.path().join("expected.rs")).unwrap();
        assert_eq!(left, right);
    }

    #[test]
    fn test_bundle_with_files_folders() {
        let temp_dir = tempdir::TempDir::new("bundle_rs").unwrap();
        std::fs::create_dir(temp_dir.path().join("game")).unwrap();
        std::fs::copy(
            "./data/test-2/game/mod.rs",
            temp_dir.path().join("game/mod.rs"),
        )
        .unwrap();
        std::fs::copy(
            "./data/test-2/game/inner.rs",
            temp_dir.path().join("game/inner.rs"),
        )
        .unwrap();
        std::fs::copy("./data/test-2/main.rs", temp_dir.path().join("main.rs")).unwrap();
        let file_system = ModuleFileSystem::new(vec![temp_dir.path().to_str().unwrap()]);
        let mut bundle = Bundle::new("main", file_system);
        bundle.load().unwrap();
        bundle
            .write(&mut std::fs::File::create(temp_dir.path().join("expected.rs")).unwrap())
            .unwrap();
        println!("{}", temp_dir.path().to_str().unwrap());
        let left = std::fs::read_to_string("./data/test-2/expected.txt").unwrap();
        let right = std::fs::read_to_string(temp_dir.path().join("expected.rs")).unwrap();
        assert_eq!(left, right);
    }
}
