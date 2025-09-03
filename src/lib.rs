use std::{
    io,
    fs,
    path::Path,
    collections::HashMap
};
use hash::hash_file_rename;
use lightningcss::{
    printer::PrinterOptions,
    stylesheet::{
        MinifyOptions,
        ParserOptions,
        StyleSheet
    }
};
use thiserror::Error;

pub mod hash;

#[derive(Error, Debug)]
pub enum StaticPreprocessingError {
    #[error("There was an error accessing I/O: {0}")]
    IOError(#[from] io::Error),
    #[error("There was a parsing error: {0}")]
    ParsingError(String),
    #[error("There was an error during minification: {0}")]
    MinificationError(String),
    #[error("There was an error during hashing: {0}")]
    HashError(String),
    #[error("There was an error during Image processing: {0}")]
    ImageProcessingError(String)
}

type LibError = StaticPreprocessingError;

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
/// writeln!(file, "body {{ background: #fff; }}").unwrap();
///
/// let loaded = load_file(&path).unwrap();
/// assert_eq!(loaded.filename, path.file_name().unwrap().to_string_lossy());
/// assert!(loaded.file_type == FileType::CSS);
/// assert!(loaded.contents == b"body { background: #fff; }\n");
/// ```
pub fn load_file(path: &Path) -> Result<File, LibError> {
    Ok(File {
        filename: path.file_name().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid file name."))?.to_string_lossy().to_string(),
        file_type: path
            .extension()
            .and_then(|ext| ext.to_str())
            .ok_or_else(|| LibError::IOError(io::Error::new(io::ErrorKind::InvalidInput, "Invalid file extension.")))
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
pub fn save_file(output_dir: &Path, file: &File) -> Result<(), LibError> {
    fs::write(output_dir.join(&file.filename), &file.contents).map_err(|err| LibError::IOError(err))
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
pub fn process_directory(input_dir: &Path, output_dir: &Path) -> Result<(), LibError> {
    fs::create_dir_all(output_dir)?;

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
) -> Result<(), LibError> {
    let input_file = load_file(path)?;

    let minified_css = minify_css(input_file);
    
    let hashed_file = hash_file_rename(minified_css?)?;

    manifest.insert(
        path.to_string_lossy().to_string(),
        hashed_file.filename.clone(),
    );

    save_file(output_dir, &hashed_file)
}

fn minify_css(f: File) ->  Result<File, LibError> {
    if f.file_type != FileType::CSS {
        return Ok(f);
    }
    
    let contents = std::str::from_utf8(&f.contents)
        .map_err(|err| LibError::ParsingError(err.to_string()))?;

    let mut ss = StyleSheet::parse(contents, ParserOptions::default())
        .map_err(|err| LibError::ParsingError(err.to_string()))?;

    ss.minify(MinifyOptions::default())
        .map_err(|err| LibError::MinificationError(err.to_string()))?;

    let minified_contents = ss.to_css(PrinterOptions { minify: true, ..PrinterOptions::default() })
        .map_err(|err| LibError::MinificationError(err.to_string()))?
        .code
        .into_bytes();
    
    Ok(File {
        contents: minified_contents,
        ..f
    })
}

/// Writes the manifest file to the output directory as pretty-printed JSON.
fn write_manifest(output_dir: &Path, manifest: &HashMap<String, String>) -> Result<(), LibError> {
    let manifest_path = output_dir.join("manifest.json");
    let json = serde_json::to_string_pretty(manifest)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    fs::write(manifest_path, json).map_err(|err| LibError::IOError(err))
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
pub fn for_each_file<F: FnMut(&Path) -> Result<(), LibError>>(path: &Path, f: &mut F) -> Result<(), LibError> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_file_type() {
        assert!(detect_file_type("css") == FileType::CSS);
        assert!(detect_file_type("js") == FileType::JS);
        let img_types = Vec::from(["webp", "jpg", "jpeg", "png", "avif"]);
        for img_type in img_types {
            assert!(detect_file_type(img_type) == FileType::Image);
        }
    }

    #[test]
    fn test_detect_file_type_negative() {
        let non_recognized_types = Vec::from(["gif", "tiff", "docx", "thing", "stl", "a", "file", "txt"]);
        for non_type in non_recognized_types {
            assert!(detect_file_type(non_type) == FileType::Other);
        }
    }

    #[test]
    fn test_load_file() {
        use std::fs::File as FsFile;
        use std::io::Write;
        use tempfile::tempdir;
        
        let dir = tempdir().unwrap();
        let path = dir.path().join("example.css");
        let mut file = FsFile::create(&path).unwrap();
        writeln!(file, "body {{ background: #fff; }}").unwrap();

        let loaded = load_file(&path).unwrap();
        assert_eq!(loaded.filename, path.file_name().unwrap().to_string_lossy());
        assert!(loaded.file_type == FileType::CSS);
        assert!(loaded.contents == b"body { background: #fff; }\n");
    }

    #[test]
    fn test_load_file_negative() {
        let res = load_file(Path::new("non/existant/thispathdefinatelyhouldnevereverexistanywhere/path.file"));
        if let Ok(_) = res {
            panic!("Invalid file loaded");
        }
    }

    #[test]
    fn test_save_file() {
        use std::fs;
        use tempfile::tempdir;
        
        let dir = tempdir().unwrap();
        let file = File {
            filename: "hello.txt".into(),
            file_type: FileType::Other,
            contents: b"Hello, world!".to_vec(),
        };
        save_file(dir.path(), &file).unwrap();
        let written = fs::read_to_string(dir.path().join("hello.txt")).unwrap();
        assert_eq!(written, "Hello, world!");
    }

    #[test]
    fn test_write_manifest() {
        use std::fs;
        use tempfile::tempdir;

        let dir = tempdir().unwrap();
        let output_dir = dir.path();

        // Create a sample manifest
        let mut manifest = HashMap::new();
        manifest.insert(
            "/input/example.css".to_string(),
            "example-hashed.css".to_string(),
        );
        manifest.insert(
            "/input/script.js".to_string(),
            "script-hashed.js".to_string(),
        );

        // Write the manifest to the output directory
        write_manifest(output_dir, &manifest).unwrap();

        // Read the manifest file back
        let manifest_path = output_dir.join("manifest.json");
        let written_manifest = fs::read_to_string(manifest_path).unwrap();

        // Verify the contents of the manifest file
        let expected_manifest = serde_json::to_string_pretty(&manifest).unwrap();
        assert_eq!(written_manifest, expected_manifest);
    }

    #[test]
    fn test_for_each_file() {
        use std::fs::{self, File as FsFile};
        use std::io::Write;
        use tempfile::tempdir;

        // Create a temporary directory with some files and subdirectories
        let temp = tempdir().unwrap();
        let file1_path = temp.path().join("file1.txt");
        let file2_path = temp.path().join("subdir").join("file2.txt");

        // Create the files
        fs::create_dir(temp.path().join("subdir")).unwrap();
        let mut file1 = FsFile::create(&file1_path).unwrap();
        writeln!(file1, "File 1 contents").unwrap();
        let mut file2 = FsFile::create(&file2_path).unwrap();
        writeln!(file2, "File 2 contents").unwrap();

        // Use for_each_file to count the files
        let mut count = 0;
        for_each_file(temp.path(), &mut |path| {
            if path.is_file() {
                count += 1;
            }
            Ok(())
        })
        .unwrap();

        // Verify that both files were counted
        assert_eq!(count, 2);
    }

    #[test]
    fn test_minify_css() {
        use std::str;

        let input_file = File {
            filename: "example.css".into(),
            file_type: FileType::CSS,
            contents: b"body { color: red; }  /* comment */".to_vec(),
        };

        let result = minify_css(input_file).unwrap();

        assert_eq!(result.file_type, FileType::CSS);
        assert_eq!(result.filename, "example.css");
        assert!(str::from_utf8(&result.contents).unwrap().contains("body{color:red}"));
    }

    #[test]
    fn test_minify_css_non_css_file() {
        let input_file = File {
            filename: "example.txt".into(),
            file_type: FileType::Other,
            contents: b"Some random text".to_vec(),
        };

        let result = minify_css(input_file).unwrap();

        assert_eq!(result.file_type, FileType::Other);
        assert_eq!(result.filename, "example.txt");
        assert_eq!(result.contents, b"Some random text");
    }
}