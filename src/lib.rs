use lazy_static::lazy_static;
use std::writeln;
use std::{
    collections::hash_set::HashSet, collections::HashMap, fs::File, io::BufRead, io::ErrorKind,
    io::Write, path::Path,
};

pub mod syntax;

pub struct ModuleFileSystem<'a> {
    search_dirs: Vec<&'a str>,
}

impl<'a> ModuleFileSystem<'a> {
    pub fn new(paths: Vec<&'a str>) -> Self {
        Self { search_dirs: paths }
    }
}

impl<'a> FileSystem for ModuleFileSystem<'a> {
    type Reader = File;
    fn open_submodule(
        &self,
        relative_path: &str,
        module_name: &str,
    ) -> std::io::Result<Self::Reader> {
        let search_dirs: Vec<String> = self
            .search_dirs
            .iter()
            .map(|src| {
                vec![
                    format!("{}{}/{}.rs", src, relative_path, module_name),
                    format!("{}{}/{}/mod.rs", src, relative_path, module_name),
                ]
            })
            .flatten()
            .collect();
        println!("{:?}", search_dirs);

        search_dirs
            .iter()
            .find(|path| std::fs::metadata(path).is_ok())
            .map(|x| std::fs::File::open(Path::new(&x)).unwrap())
            .ok_or(std::io::Error::new(ErrorKind::NotFound, "not found"))
    }
}

pub trait FileSystem {
    type Reader: std::io::Read;
    fn open_submodule(
        &self,
        relative_path: &str,
        submodule_name: &str,
    ) -> std::io::Result<Self::Reader>;
}

pub struct Bundle<'a, F>
where
    F: FileSystem,
{
    entry_module: &'a str,
    loaded_tokens: Vec<syntax::LineToken>,
    file_system: F,
}

impl<'a, F: FileSystem> Bundle<'a, F>
where
    F: FileSystem,
{
    pub fn new(entry_module: &'a str, file_system: F) -> Self {
        Self {
            entry_module: entry_module,
            loaded_tokens: Vec::new(),
            file_system,
        }
    }

    pub fn refactor(&mut self) {}

    fn write_tokens<W: std::io::Write>(
        writer: &mut std::io::BufWriter<W>,
        tokens: &Vec<syntax::LineToken>,
    ) -> std::io::Result<()> {
        for token in tokens.iter() {
            match token {
                syntax::LineToken::UseModule { line, name: _ }
                | syntax::LineToken::UseManyModules {
                    line,
                    parent: _,
                    names: _,
                }
                | syntax::LineToken::DeclareOtherModule { line, name: _ }
                | syntax::LineToken::OtherLine {
                    line,
                    trimmed_ref: _,
                } => {
                    writer.write(line.as_bytes())?;
                    writer.write("\n".as_bytes())?;
                }
                syntax::LineToken::Module {
                    name,
                    is_pub,
                    tokens,
                } => {
                    if is_pub == &true {
                        writer.write("pub ".as_bytes())?;
                    }
                    writeln!(writer, "mod {}{{", name)?;
                    Self::write_tokens(writer, tokens)?;
                    writer.write("}\n".as_bytes())?;
                }
            }
        }
        Ok(())
    }

    pub fn write<W: std::io::Write>(&self, write: &mut W) -> std::io::Result<()> {
        let mut buf_writer = std::io::BufWriter::new(write);
        Self::write_tokens(&mut buf_writer, &self.loaded_tokens)
    }

    fn load_tokens<R: BufRead>(
        &self,
        relative_path: String,
        reader: R,
    ) -> std::io::Result<Vec<syntax::LineToken>> {
        let mut result = Vec::<syntax::LineToken>::new();
        for line in reader.lines() {
            match syntax::parse_line(line?) {
                syntax::LineToken::DeclareOtherModule { line, name } => {
                    let module_name = name.resolve_unchecked(line.as_str());

                    let inner_reader = std::io::BufReader::new(
                        self.file_system
                            .open_submodule(relative_path.as_str(), module_name)?,
                    );
                    let relative_path = format!("{}/{}", relative_path, module_name);
                    let module = self.load_tokens(relative_path, inner_reader)?;

                    result.push(syntax::LineToken::Module {
                        name: module_name.to_string(),
                        is_pub: true,
                        tokens: module,
                    })
                }
                t => result.push(t),
            }
        }
        Ok(result)
    }

    pub fn load(&mut self) -> std::io::Result<()> {
        let reader =
            std::io::BufReader::new(self.file_system.open_submodule("", self.entry_module)?);
        self.loaded_tokens = self.load_tokens(String::from(""), reader)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use syntax::LineToken;

    use crate::syntax::LineRef;

    use super::*;

    struct HashMapFiles<'a> {
        file_map: HashMap<&'a str, &'a [u8]>,
    }

    impl<'a> HashMapFiles<'a> {
        fn new() -> Self {
            Self {
                file_map: HashMap::new(),
            }
        }

        fn insert(&mut self, name: &'a str, content: &'a [u8]) {
            self.file_map.insert(name, content);
        }
    }

    impl<'a> FileSystem for HashMapFiles<'a> {
        type Reader = &'a [u8];

        fn open_submodule(
            &self,
            relative_path: &str,
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
        let mut map = HashMapFiles::new();
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
            bundle.loaded_tokens
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
