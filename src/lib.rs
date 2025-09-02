use std::collections::HashMap;
use std::path::Path;
use std::io;
use std::fs;
use hash::hash_file_rename;
use lightningcss::printer::PrinterOptions;
use lightningcss::stylesheet::MinifyOptions;
use lightningcss::stylesheet::ParserOptions;
use lightningcss::stylesheet::StyleSheet;

pub mod hash;

#[derive(Debug, PartialEq)]
pub enum FileType {
    Image,
    CSS,
    JS,
    Other
}

#[derive(Debug)]
pub struct File {
    /// The file's name (not including any directory).
    pub filename: String,
    /// The detected type of the file.
    pub file_type: FileType,
    /// The raw file contents.
    pub contents: Vec<u8>
}

/// Loads a file from disk and constructs a [`File`] with its metadata and contents.
///
/// This function reads a file at the given path and returns a [`File`] containing:
/// - The filename (not the full path)
/// - The file contents
/// - The inferred [`FileType`] based on the file extension
///
/// # Parameters
///
/// - `path`: The path to the file.
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
/// writeln!(file, "body { background: #fff; }").unwrap();
///
/// let loaded = load_file(&path).unwrap();
/// assert_eq!(loaded.filename, "example.css");
/// assert!(matches!(loaded.file_type, FileType::CSS));
/// assert_eq!(loaded.contents, b"body { background: #fff; }\n");
/// ```
pub fn load_file(path: &Path) -> Result<File, io::Error> {
    Ok(File {
        filename: path.file_name().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid file name."))?.to_string_lossy().to_string(),
        file_type: path
            .extension()
            .and_then(|ext| ext.to_str())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid file extension."))
            .map(detect_file_type)?,
        contents: fs::read(path)?
    })
}

/// Writes the contents of a [`File`] to disk in the specified output directory.
///
/// The file will be saved as `output_dir/filename`. If the file already exists, it will be overwritten.
///
/// # Parameters
///
/// - `output_dir`: The directory to write the file into.
/// - `file`: The [`File`] to be saved.
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
///     filename: "hello.txt".into(),
///     file_type: FileType::Other,
///     contents: b"Hello, world!".to_vec(),
/// };
///
/// save_file(dir.path(), &file).unwrap();
///
/// let written = fs::read_to_string(dir.path().join("hello.txt")).unwrap();
/// assert_eq!(written, "Hello, world!");
/// ```
pub fn save_file(output_dir: &Path, file: &File) -> Result<(), io::Error> {
    fs::write(output_dir.join(&file.filename), &file.contents)
}

/// Processes all files in a directory tree and writes them to an output directory with hashed filenames.
///
/// This function recursively traverses `input_dir`, loading each file, hashing its contents,
/// and saving it under a new filename based on the BLAKE3 hash.  
/// **Note:** The output directory will contain all processed files at its root (no subdirectories).
///
/// Additionally, a `manifest.json` file is created in the `output_dir`, mapping each original file's full path
/// (as a string) to its hashed filename.
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
/// The `manifest.json` file contains a JSON object where each key is the original full path
/// of a file (as a string), and the value is the hashed filename (relative to `output_dir`).
///
/// # Examples
///
/// ```
/// # use std::fs::{self, File as FsFile};
/// # use std::io::Write;
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
        process_file(path, output_dir, &mut manifest)
    })?;

    write_manifest(output_dir, &manifest)?;

    Ok(())
}

/// Processes a single file: loads it, hashes its name, and saves it to the output directory.
fn process_file(
    path: &Path,
    output_dir: &Path,
    manifest: &mut HashMap<String, String>,
) -> Result<(), io::Error> {
    let input_file = load_file(path)?;

    let minified_file = match input_file.file_type {
        FileType::CSS => {
            let mut ss = StyleSheet::parse(std::str::from_utf8(&input_file.contents).unwrap(), ParserOptions::default()).unwrap();
            ss.minify(MinifyOptions::default()).unwrap();
            File {
                contents: ss.to_css(PrinterOptions{minify: true, ..PrinterOptions::default()}).unwrap().code.into_bytes(),
                ..input_file
            }
        },
        _ => input_file
    };
    
    let hashed_file = hash_file_rename(minified_file)?;

    manifest.insert(
        path.to_string_lossy().to_string(),
        hashed_file.filename.clone(),
    );

    save_file(output_dir, &hashed_file)
}

/// Writes the manifest file to the output directory as pretty-printed JSON.
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
/// # use static_preprocessing::detect_file_type;
/// # use static_preprocessing::FileType;
/// assert!(matches!(detect_file_type("css"), FileType::CSS));
/// assert!(matches!(detect_file_type("png"), FileType::Image));
/// assert!(matches!(detect_file_type("txt"), FileType::Other));
/// ```
pub fn detect_file_type(ext: &str) -> FileType {
    match ext {
        "css" => FileType::CSS,
        "js" => FileType::JS,
        "webp" | "jpg" | "jpeg" | "png" | "avif" => FileType::Image,
        _ => FileType::Other,
    }
}