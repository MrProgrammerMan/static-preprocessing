use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::io;
use std::fs;
use hash::hash_file_rename;

pub mod hash;

#[derive(Debug, PartialEq)]
pub enum FileType {
    Image,
    CSS,
    JS,
    Other
}

#[derive(Debug)]
pub struct File<'a> {
    pub parent: &'a Path,
    pub relative_path: PathBuf,
    pub file_type: FileType,
    pub contents: Vec<u8>
}

impl File<'_> {
    pub fn total_relative_path(&self) -> PathBuf {
        self.parent.join(&self.relative_path)
    }
}

/// Loads a file from disk and constructs a [`File`] with its metadata and contents.
///
/// This function reads a file located at `parent.join(relative_path)` and returns
/// a [`File`] containing:
/// - The `parent` base directory
/// - The `relative_path`
/// - The file contents
/// - The inferred [`FileType`] based on the file extension
///
/// # Parameters
///
/// - `parent`: The base directory for the file.
/// - `relative_path`: The path to the file relative to `parent`.
///
/// # Returns
///
/// [`Ok`] containing the fully populated [`File`] on success, or an [`io::Error`] if:
/// - The file extension is missing or invalid
/// - The file could not be read from disk
///
/// # Examples
///
/// ```
/// # use std::fs::File as FsFile;
/// # use std::io::Write;
/// # use std::path::Path;
/// # use tempfile::tempdir;
/// # use static_preprocessing::{File, FileType, load_file};
/// #
/// let dir = tempdir().unwrap();
/// let path = dir.path().join("example.css");
/// let mut file = FsFile::create(&path).unwrap();
/// writeln!(file, "body {{ background: #fff; }}").unwrap();
///
/// let loaded = load_file(dir.path(), Path::new("example.css")).unwrap();
/// assert!(matches!(loaded.file_type, FileType::CSS));
/// assert_eq!(loaded.contents, b"body { background: #fff; }\n");
/// ```
///
/// [`File`]: struct.File.html
/// [`FileType`]: enum.FileType.html
/// [`Ok`]: https://doc.rust-lang.org/std/result/enum.Result.html#variant.Ok
/// [`io::Error`]: https://doc.rust-lang.org/std/io/struct.Error.html
pub fn load_file<'a>(parent: &'a Path, relative_path: &Path) -> Result<File<'a>, io::Error> {
    Ok(File {
        parent,
        relative_path: relative_path.to_path_buf(),
        file_type: relative_path
            .extension()
            .and_then(|ext| ext.to_str())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid file extension."))
            .map(detect_file_type)?,
        contents: fs::read(parent.join(&relative_path))?
    })
}

/// Writes the contents of a [`File`] to disk at its computed full path.
///
/// This function saves the file’s `contents` to the filesystem using the full path
/// resolved by [`File::total_relative_path`]. If the file already exists, it will be overwritten.
///
/// # Parameters
///
/// - `file`: A reference to the [`File`] to be saved. The file’s `parent` and `relative_path`
///   determine the target location.
///
/// # Returns
///
/// [`Ok`] if the write succeeds, or an [`io::Error`] if writing to disk fails.
///
/// # Examples
///
/// ```
/// # use std::fs;
/// # use std::path::Path;
/// # use tempfile::tempdir;
/// # use static_preprocessing::{File, FileType, save_file};
/// #
/// let dir = tempdir().unwrap();
/// let file = File {
///     parent: dir.path(),
///     relative_path: "hello.txt".into(),
///     file_type: FileType::Other,
///     contents: b"Hello, world!".to_vec(),
/// };
///
/// save_file(&file).unwrap();
///
/// let written = fs::read_to_string(dir.path().join("hello.txt")).unwrap();
/// assert_eq!(written, "Hello, world!");
/// ```
///
/// [`File`]: struct.File.html
/// [`File::total_relative_path`]: struct.File.html#method.total_relative_path
/// [`Ok`]: https://doc.rust-lang.org/std/result/enum.Result.html#variant.Ok
/// [`io::Error`]: https://doc.rust-lang.org/std/io/struct.Error.html
pub fn save_file(file: &File) -> Result<(), io::Error> {
    fs::write(file.total_relative_path(), &file.contents)
}

/// Processes all files in a directory tree and writes them to an output directory with hashed filenames.
///
/// This function recursively traverses `input_dir`, loading each file, hashing its contents,
/// and saving it under a new filename based on the BLAKE3 hash. The original directory structure
/// is preserved in `output_dir`, and all necessary subdirectories are created automatically.
///
/// Additionally, a `manifest.json` file is created in the `output_dir`, mapping original filenames
/// (relative to `input_dir`) to their hashed filenames.
///
/// # Parameters
///
/// - `input_dir`: The root input directory to scan recursively.
/// - `output_dir`: The root output directory where processed files are saved.
///
/// # Returns
///
/// [`Ok`] if all files were processed successfully, or an [`io::Error`] if any file or directory operation fails.
///
/// # Manifest File
///
/// The `manifest.json` file contains a JSON object where each key is the original relative path
/// of a file (from `input_dir`), and the value is the hashed filename (relative to `output_dir`).
///
/// # Examples
///
/// ```
/// # use std::fs::{self, File as FsFile};
/// # use std::io::Write;
/// # use std::path::Path;
/// # use tempfile::tempdir;
/// # use static_preprocessing::process_directory;
/// #
/// let input_dir = tempdir().unwrap();
/// let output_dir = tempdir().unwrap();
///
/// let file_path = input_dir.path().join("example.txt");
/// let mut file = FsFile::create(&file_path).unwrap();
/// writeln!(file, "static content").unwrap();
///
/// process_directory(input_dir.path(), output_dir.path()).unwrap();
///
/// // The output_dir should now contain a hashed version of "example.txt"
/// let entries: Vec<_> = fs::read_dir(output_dir.path())
///     .unwrap()
///     .flat_map(|res| res.ok())
///     .collect();
///
/// assert!(!entries.is_empty());
/// ```
pub fn process_directory(input_dir: &Path, output_dir: &Path) -> Result<(), io::Error> {
    fs::create_dir(output_dir)?;

    let mut manifest = HashMap::new();

    for_each_file(input_dir, &mut |path| {
        process_file(path, input_dir, output_dir, &mut manifest)
    })?;

    write_manifest(output_dir, &manifest)?;

    Ok(())
}

/// Processes a single file: loads it, hashes its name, and saves it to the output directory.
fn process_file(
    path: &Path,
    input_dir: &Path,
    output_dir: &Path,
    manifest: &mut HashMap<String, String>,
) -> Result<(), io::Error> {
    let relative_path = path
        .strip_prefix(input_dir)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Invalid file"))?;
    
    let input_file = load_file(input_dir, relative_path)?;
    
    let hashed_file = hash_file_rename(input_file)?;

    let output_file = File {
        parent: output_dir,
        ..hashed_file
    };

    manifest.insert(
        relative_path.to_string_lossy().to_string(),
        output_file.relative_path.to_string_lossy().to_string(),
    );

    save_file(&output_file)
}

/// Writes the manifest file to the output directory.
fn write_manifest(output_dir: &Path, manifest: &HashMap<String, String>) -> Result<(), io::Error> {
    let manifest_path = output_dir.join("manifest.json");
    let json = serde_json::to_string_pretty(manifest)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    fs::write(manifest_path, json)
}

/// Recursively traverses a directory tree, applying a function to each file found.
///
/// This function walks through the directory at `path` and all its subdirectories,
/// calling the provided closure `f` on every file encountered. If `path` itself is a
/// file, the closure is applied directly.
///
/// # Parameters
///
/// - `path`: The starting directory or file path to traverse.
/// - `f`: A mutable closure that takes a reference to a [`Path`] and returns an [`io::Result<()>`].
///
/// # Returns
///
/// [`Ok(())`] if all files were processed successfully, or the first [`io::Error`] encountered.
///
/// # Examples
///
/// ```
/// # use std::fs::{self, File as FsFile};
/// # use std::io::Write;
/// # use std::path::Path;
/// # use tempfile::tempdir;
/// # use static_preprocessing::for_each_file;
/// #
/// let temp = tempdir().unwrap();
/// let file_path = temp.path().join("foo.txt");
/// let mut file = FsFile::create(&file_path).unwrap();
/// writeln!(file, "Hello").unwrap();
///
/// let mut count = 0;
/// for_each_file(temp.path(), &mut |path| {
///     if path.is_file() {
///         count += 1;
///     }
///     Ok(())
/// }).unwrap();
///
/// assert_eq!(count, 1);
/// ```
///
/// [`Path`]: https://doc.rust-lang.org/std/path/struct.Path.html
/// [`Ok(())`]: https://doc.rust-lang.org/std/result/enum.Result.html#variant.Ok
/// [`io::Error`]: https://doc.rust-lang.org/std/io/struct.Error.html
pub fn for_each_file<F: FnMut(&Path) -> io::Result<()>>(path: &Path, f: &mut F) -> io::Result<()> {
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            for_each_file(&path, f)?;
        }
        Ok(())
    } else {
        f(path)
    }
}

/// Determines the [`FileType`] based on the file extension.
///
/// This function maps common file extensions to specific [`FileType`] variants.
/// It recognizes `"css"`, `"js"`, and common image formats like `"webp"`, `"jpg"`, `"jpeg"`, `"png"`, and `"avif"`.
/// All other extensions are classified as [`FileType::Other`].
///
/// # Parameters
///
/// - `ext`: A string slice representing the file extension (without the dot).
///
/// # Returns
///
/// A corresponding variant of [`FileType`] based on the extension.
///
/// # Examples
///
/// ```
/// # use std::path::Path;
/// # use static_preprocessing::detect_file_type;
/// # use static_preprocessing::FileType;
/// assert!(matches!(detect_file_type("css"), FileType::CSS));
/// assert!(matches!(detect_file_type("png"), FileType::Image));
/// assert!(matches!(detect_file_type("txt"), FileType::Other));
/// ```
///
/// [`FileType`]: enum.FileType.html
pub fn detect_file_type(ext: &str) -> FileType {
    match ext {
        "css" => FileType::CSS,
        "js" => FileType::JS,
        "webp" | "jpg" | "jpeg" | "png" | "avif" => FileType::Image,
        _ => FileType::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File as FsFile};
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_load_file_valid_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("example.css");
        let mut file = FsFile::create(&file_path).unwrap();
        writeln!(file, "body {{ background: #fff; }}").unwrap();

        let loaded = load_file(dir.path(), Path::new("example.css")).unwrap();
        assert_eq!(loaded.relative_path, Path::new("example.css"));
        assert!(matches!(loaded.file_type, FileType::CSS));
        assert_eq!(loaded.contents, b"body { background: #fff; }\n");
    }

    #[test]
    fn test_load_file_invalid_extension() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("example");
        let mut file = FsFile::create(&file_path).unwrap();
        writeln!(file, "content").unwrap();

        let result = load_file(dir.path(), Path::new("example"));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn test_save_file() {
        let dir = tempdir().unwrap();
        let file = File {
            parent: dir.path(),
            relative_path: "hello.txt".into(),
            file_type: FileType::Other,
            contents: b"Hello, world!".to_vec(),
        };

        save_file(&file).unwrap();
        let written = fs::read_to_string(dir.path().join("hello.txt")).unwrap();
        assert_eq!(written, "Hello, world!");
    }

    #[test]
    fn test_create_dir_structure() {
        let dir = tempdir().unwrap();
        let file = File {
            parent: Path::new("input"),
            relative_path: Path::new("nested/dir/file.txt").to_path_buf(),
            file_type: FileType::Other,
            contents: b"example".to_vec(),
        };

        create_dir_structure(dir.path(), &file).unwrap();
        let created_path = dir.path().join("nested/dir");
        assert!(created_path.exists());
    }

    #[test]
    fn test_for_each_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("foo.txt");
        let mut file = FsFile::create(&file_path).unwrap();
        writeln!(file, "Hello").unwrap();

        let mut count = 0;
        for_each_file(dir.path(), &mut |path| {
            if path.is_file() {
                count += 1;
            }
            Ok(())
        })
        .unwrap();

        assert_eq!(count, 1);
    }

    #[test]
    fn test_detect_file_type() {
        assert!(matches!(detect_file_type("css"), FileType::CSS));
        assert!(matches!(detect_file_type("js"), FileType::JS));
        assert!(matches!(detect_file_type("png"), FileType::Image));
        assert!(matches!(detect_file_type("txt"), FileType::Other));
    }

    #[test]
    fn test_process_directory() {
        let input_dir = tempdir().unwrap();
        let output_dir = tempdir().unwrap();

        let file_path = input_dir.path().join("example.txt");
        let mut file = FsFile::create(&file_path).unwrap();
        writeln!(file, "static content").unwrap();

        process_directory(input_dir.path(), output_dir.path()).unwrap();

        let entries: Vec<_> = fs::read_dir(output_dir.path())
            .unwrap()
            .flat_map(|res| res.ok())
            .collect();

        assert!(!entries.is_empty());
    }
}