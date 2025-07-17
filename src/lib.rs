use std::path::{Path, PathBuf};
use std::io;
use std::fs;

/// Recursively applies a function to each file in a directory tree.
///
/// This function traverses the directory specified by `p` and recursively
/// visits all its subdirectories. For each regular file encountered,
/// the provided closure `f` is called with a reference to the file's [`Path`].
///
/// # Arguments
///
/// * `p` - A reference to a [`Path`] representing the root directory to traverse.
/// * `f` - A mutable reference to a closure that takes a reference to a [`Path`] and returns `()`
///         (i.e., performs side effects such as logging, collecting paths, etc.).
///
/// # Returns
///
/// * [`Ok(())`] if the traversal completes successfully.
/// * [`Err`] if `p` is not a directory or if any I/O error occurs while reading directories.
///
/// # Errors
///
/// Returns an [`io::ErrorKind::NotADirectory`] error if `p` is not a directory.
/// Also propagates any errors returned by [`fs::read_dir`] or while accessing entries.
///
/// # Example
/// ```
///  # use std::fs;
///  # use std::path::PathBuf;
///  # use tempfile::TempDir;
///  # use static_preprocessing::for_each_file;
///  # /// Create temporary directory
///  # let temp_dir = TempDir::new().unwrap();
///  # let file0 = temp_dir.path().join("file0.txt");
///  # let sub_dir = temp_dir.path().join("sub");
///  # let sub_file = sub_dir.join("file1.jpeg");
///
///  # /// Create files
///  # fs::File::create(&file0).unwrap();
///  # fs::create_dir(&sub_dir).unwrap();
///  # fs::File::create(&sub_file).unwrap();
///
///  /// Capture visited paths
///  let mut visited: Vec<PathBuf> = Vec::new();
///
///  for_each_file(temp_dir.path(), &mut |p| Ok(visited.push(p.to_path_buf()))).unwrap();
///
///  /// Check all files were visited
///  assert!(visited.contains(&file0));
///  assert!(visited.contains(&sub_file));
///  assert_eq!(visited.len(), 2);
/// ```
///
/// [`Path`]: std::path::Path
/// [`Ok(())`]: std::result::Result::Ok
/// [`Err`]: std::result::Result::Err
/// [`fs::read_dir`]: std::fs::read_dir
/// [`io::ErrorKind::NotADirectory`]: std::io::ErrorKind::NotADirectory
pub fn for_each_file<F: FnMut(&Path) -> io::Result<()>>(dir: &Path, f: &mut F) -> io::Result<()> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_dir() {
                for_each_file(&path, f)?;
            } else {
                f(&path)?;
            }
        }
        Ok(())
    } else {
        Err(io::Error::new(io::ErrorKind::NotADirectory, "Not a directory"))
    }
}

/// Computes a hashed filename for a given file.
///
/// This function generates a Blake3 hash of the file contents and combines it with
/// the original file extension to create a unique filename.
///
/// # Arguments
///
/// * `path` - A reference to the original file's [`Path`], used to extract the extension.
/// * `file_contents` - A byte slice containing the file's contents to be hashed.
///
/// # Returns
///
/// A [`String`] containing the hash value followed by the original file extension.
/// If the file has no extension or the extension is not valid UTF-8, an empty string
/// is used as the extension.
///
/// # Example
///
/// ```
///  # use std::path::Path;
///  # use static_preprocessing::compute_hashed_filename;
///  let contents = "This is the contents of a text file.".as_bytes();
///  let path = Path::new("file.txt");
///  let hash = compute_hashed_filename(path, contents);
///  assert_eq!(hash.as_str(), "54d6c1211787e4f3e257ba41a3abf553436f578e93ba56555945c7222932733c.txt");
/// ```
///
/// [`Path`]: std::path::Path
/// [`String`]: std::string::String
pub fn compute_hashed_filename(path: &Path, file_contents: &[u8]) -> String {
    let hash = blake3::hash(file_contents).to_string();
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    format!("{}.{}", hash, ext)
}

/// Creates an output directory structure mirroring the original file's location.
///
/// Given an output base directory and a source file path, this function creates
/// a directory structure in the output directory that matches the source file's
/// directory hierarchy.
///
/// # Arguments
///
/// * `output_dir` - A reference to the base [`Path`] for output files.
/// * `file_path` - A reference to the original file's [`Path`].
///
/// # Returns
///
/// * [`Ok(PathBuf)`] containing the complete output directory path where the processed file should be saved.
/// * [`Err`] if directory creation fails.
///
/// # Errors
///
/// This function will propagate any I/O errors encountered during directory creation.
///
/// # Example
///
/// ```
/// # use std::path::Path;
/// # use static_preprocessing::create_output_directory;
/// let output_dir = Path::new("output");
/// let file_path = Path::new("static_files/css/main.css");
/// create_output_directory(output_dir, file_path).unwrap();
/// ```
///
/// [`Path`]: std::path::Path
/// [`PathBuf`]: std::path::PathBuf
/// [`Ok(PathBuf)`]: std::result::Result::Ok
/// [`Err`]: std::result::Result::Err
pub fn create_output_directory(output_dir: &Path, file_path: &Path) -> io::Result<PathBuf> {
    let relative_parent = file_path.parent().unwrap_or_else(|| Path::new(""));
    let output_path = output_dir.join(relative_parent);
    fs::create_dir_all(&output_path)?;
    Ok(output_path)
}

/// Processes all files in a directory tree, creating hashed copies in the output directory.
///
/// This function traverses the input directory recursively and processes each file,
/// maintaining the original directory structure in the output location.
///
/// # Arguments
///
/// * `input_dir` - A reference to the [`Path`] of the directory to process.
/// * `output_dir` - A reference to the [`Path`] where processed files will be written.
///
/// # Returns
///
/// * [`Ok(())`] if all files were processed successfully.
/// * [`Err`] if any file processing operation fails.
///
/// # Errors
///
/// This function will return the first error encountered during directory traversal
/// or file processing.
///
/// # Example
///
/// ```
/// // Example will be added here
/// ```
///
/// [`Path`]: std::path::Path
/// [`Ok(())`]: std::result::Result::Ok
/// [`Err`]: std::result::Result::Err
pub fn process_directory(input_dir: &Path, output_dir: &Path) -> io::Result<()> {
    for_each_file(input_dir, &mut |file_path| process_file(file_path, output_dir))
}

/// Processes a single file by creating a hashed copy in the output directory.
///
/// This function reads the contents of a file, computes a hash-based filename,
/// creates the necessary output directory structure, and writes the file with
/// its new name to the appropriate location.
///
/// # Arguments
///
/// * `file_path` - A reference to the [`Path`] of the file to process.
/// * `output_dir` - A reference to the base [`Path`] where processed files will be written.
///
/// # Returns
///
/// * [`Ok(())`] if the file was processed successfully.
/// * [`Err`] if any operation (reading, directory creation, or writing) fails.
///
/// # Errors
///
/// This function will propagate any I/O errors encountered during file reading,
/// directory creation, or file writing.
///
/// # Example
///
/// ```
/// // Example will be added here
/// ```
///
/// [`Path`]: std::path::Path
/// [`Ok(())`]: std::result::Result::Ok
/// [`Err`]: std::result::Result::Err
pub fn process_file(file_path: &Path, output_dir: &Path) -> io::Result<()> {
    let file_contents = fs::read(file_path)?;
    let hashed_filename = compute_hashed_filename(file_path, &file_contents);
    let output_path = create_output_directory(output_dir, file_path)?.join(hashed_filename);
    fs::write(output_path, file_contents)
}