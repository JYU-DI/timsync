# File processor design

The document describes the base design for file processing in TIMSync.

TIMSync is able to automatically transform various file types into TIM documents.
Instead of 1-to-1 conversion between files and documents, TIMSync processes
the files within the project directory and converts them into final TIM documents
which are then uploaded to the TIM server.
Such process allows great flexibility over project structure and simplifies
common patterns used in TIM documents.

## Data types

TIMSync defines two abstractions for file processing: project files and file processors.

### Project files

A project file is a file within the project directory that is recognized by TIMSync and can be processed.
In general, a project file is composed of an optional front matter and the main file contents.
The front matter is an optional YAML block at the beginning of the file that contains metadata about the file and
different settings.

The supported project files are declared by the `ProjectFile` enum:

<https://github.com/JYU-DI/timsync/blob/ce4f0e1086cf5cdd61025ecbef537edf80285181/timsync/src/project/files/project_files.rs#L14-L18>

Each project file must implement the `ProjectFileAPI` trait that defines the methods to access the file contents and
front matter:

<https://github.com/JYU-DI/timsync/blob/ce4f0e1086cf5cdd61025ecbef537edf80285181/timsync/src/project/files/project_files.rs#L39-L48>

Notes:

- Because different file types may have different syntax, each project file must implement its own way to pares and
  locate the front matter.

  TIMSync assumes that the first and last lines in front matter string are end/start delimiters; as such TIMSync ignores
  them.

  An example of a valid front matter string:

  ```
  ---
  title: Hello!
  ---
  ```

  TIMSync will automatically ignore the first and last lines and parse the content as YAML.
  It does not matter what the delimiters are specifically; each project file may implement its own parsing.

  An example of an invalid front matter string:
  ```
  ---
  title: Hello!
  ---
  
  ```

  TIMSync does not trim the front matter string and will consider the last empty line as the finishing delimiter.
  As such, ensure that the front matter string does not contain any extra lines at the end.
- Each project file must specify which file processor should be used to process the file by default.

### File processors

A file processor takes a list of project files and generates final TIM documents.
Each file processor may define its own logic for creating the final document structure.
For example, a normal Markdown file processor may generate a TIM document for each project file, while a task
file processor may generate a single TIM document that contains all tasks from the project files.

The file processor is also responsible for generating the final document contents.
TIMSync provides Handlebars for templating, but each file processor may define its own way to generate the contents.

All available file processors are defined in the `FileProcessor` enum:

<https://github.com/JYU-DI/timsync/blob/ce4f0e1086cf5cdd61025ecbef537edf80285181/timsync/src/processing/processors.rs#L21-L25>

As shown, each file processor must implement two traits: a public `FileProcessorAPI` trait and a
private `FileProcessorInternalAPI`:

<https://github.com/JYU-DI/timsync/blob/ce4f0e1086cf5cdd61025ecbef537edf80285181/timsync/src/processing/processors.rs#L29-L45>

<https://github.com/JYU-DI/timsync/blob/ce4f0e1086cf5cdd61025ecbef537edf80285181/timsync/src/processing/processors.rs#L49-L57>

The traits are separated, as the internal API is meant to be used only by TIMDocument.

## Pipeline

The main syncing pipeline is implemented in `SyncPipeline` struct:

<https://github.com/JYU-DI/timsync/blob/ce4f0e1086cf5cdd61025ecbef537edf80285181/timsync/src/commands/sync.rs#L77-L82>

The actual link between project file types and file processors is defined in `SyncPipeline::new`:

<https://github.com/JYU-DI/timsync/blob/ce4f0e1086cf5cdd61025ecbef537edf80285181/timsync/src/commands/sync.rs#L94-L104>

In general, the syncing pipeline is as follows:

1. Go through all files in the project directory
    - For each file, check file extension
    - If valid file extension, register as a project file
    - Each project file has a default processor assigned to it and a method to get the front matter as JSON
    - Register each project file to the file processor
2. Generate the final document structure
    - Go through each processor and request to provide the final document structure
    - Each processor will return a list of final TIMDocument instances that contain information about document path and
      title
3. Generate the files in TIM
    - For each TIMDocument, a document in TIM is created
    - The ID is obtained and saved in the TIMDocument instance which can be used in e.g., templating
4. Upload document contents
    - For each TIMDocument, request the document contents from the correct processor and upload them
    - Under the hood, the file processor will generate the contents of the document
    