use std::path::{Path, PathBuf};
use std::io;
use std::fs;

pub enum FileType {
    Image,
    CSS,
    JS,
    Other
}

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
/// The transformation process includes:
/// - Reading each file
/// - Hashing its contents for a unique filename
/// - Creating the appropriate output directory structure
/// - Writing the renamed file to the output directory
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
///
/// [`Ok`]: https://doc.rust-lang.org/std/result/enum.Result.html#variant.Ok
/// [`io::Error`]: https://doc.rust-lang.org/std/io/struct.Error.html
pub fn process_directory(input_dir: &Path, output_dir: &Path) -> Result<(), io::Error> {
    fs::create_dir_all(output_dir)?;
    for_each_file(input_dir, &mut |path| {
        let relative_path = path.strip_prefix(input_dir).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Invalid file"))?;
        let input_file = load_file(input_dir, relative_path)?;
        create_dir_structure(output_dir, &input_file)?;
        let hashed_file = hash_file_rename(input_file)?;
        let output_file = File {
            parent: output_dir,
            ..hashed_file
        };
        save_file(&output_file)?;
        Ok(())
    })
}

/// Ensures that the directory structure for a file exists within an output directory.
///
/// This function creates all necessary parent directories for the given [`File`]’s
/// `relative_path`, rooted at the specified `output_dir`. It is useful when preparing
/// a destination directory tree before writing files.
///
/// # Parameters
///
/// - `output_dir`: The root output directory where the file should be placed.
/// - `file`: The [`File`] whose `relative_path` determines the target location.
///
/// # Returns
///
/// An empty [`Ok`] result on success, or an [`io::Error`] if the path is invalid or
/// directory creation fails.
///
/// # Examples
///
/// ```
/// # use std::path::Path;
/// # use tempfile::tempdir;
/// # use static_preprocessing::{File, FileType, create_dir_structure};
/// #
/// let temp = tempdir().unwrap();
/// let file = File {
///     parent: Path::new("input"),
///     relative_path: Path::new("nested/dir/file.txt").to_path_buf(),
///     file_type: FileType::Other,
///     contents: b"example".to_vec(),
/// };
///
/// create_dir_structure(temp.path(), &file).unwrap();
///
/// let created_path = temp.path().join("nested/dir");
/// assert!(created_path.exists());
/// ```
///
/// [`File`]: struct.File.html
/// [`Ok`]: https://doc.rust-lang.org/std/result/enum.Result.html#variant.Ok
/// [`io::Error`]: https://doc.rust-lang.org/std/io/struct.Error.html
pub fn create_dir_structure(output_dir: &Path, file: &File) -> io::Result<()> {
    let path = output_dir.join(&file.relative_path);
    let parent = path.parent().ok_or(io::Error::new(io::ErrorKind::InvalidInput, "Invalid file"))?;
    fs::create_dir_all(parent)
}

// impure (fs::read_dir)
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

/// Renames a [`File`] based on the BLAKE3 hash of its contents.
///
/// This function calculates a BLAKE3 hash of the file's contents and uses it to
/// generate a new filename in the format `"{hash}.{ext}"`, preserving the original
/// file extension. The relative path is updated to reflect this new name while
/// keeping the parent directory intact.
///
/// # Parameters
///
/// - `file`: The [`File`] instance to rename. Ownership is taken since the returned
///   value modifies the original struct.
///
/// # Returns
///
/// A new [`File`] with an updated `relative_path`, or an [`io::Error`] if the file's
/// extension or parent directory is invalid.
///
/// # Examples
///
/// ```
/// # use std::path::{Path, PathBuf};
/// # use static_preprocessing::{File, FileType, hash_file_rename};
/// #
/// let file = File {
///     parent: Path::new("assets"),
///     relative_path: "css/main.css".into(),
///     file_type: FileType::CSS,
///     contents: b"body { margin: 0; }".to_vec(),
/// };
///
/// let renamed = hash_file_rename(file).unwrap();
/// assert!(renamed.relative_path.to_string_lossy().ends_with(".css"));
/// assert_eq!(renamed.relative_path, Path::new("css/057b37e61c8ec35690e7c0c321591990d37b9bdbef645cd780795a95672d65c0.css"));
/// ```
///
/// [`File`]: struct.File.html
/// [`io::Error`]: https://doc.rust-lang.org/std/io/struct.Error.html
pub fn hash_file_rename(file: File) -> Result<File, io::Error> {
    let hash = blake3::hash(file.contents.as_slice());
    let ext = file.relative_path
        .extension()
        .and_then(|ext| ext.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid file extension."))?;
    let new_name = format!("{}.{}", hash.to_string(), ext);
    let new_relative = file.relative_path
        .parent()
        .map(|parent| parent.join(new_name))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "No parent directory"))?;
    Ok(File {
        relative_path: new_relative,
        ..file
    })
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
