# File processor design

The document describes the base design for file processing in tim_sync.

## Draft

- ProjectFile( path: str ) => maybe Trait
    - DocumentFile
        - defaultExtensions: [".md"]
    - TaskFile
        - defaultExtensions: [".task.yml"]
    - StyleFile
        - defaultExtensions: [".css", ".scss"]

- FileProcessor
    - TIMDocumentProcessor
    - TaskProcessor
    - StyleProcessor

API:

- FileProcessor
    - add(file: ProjectFile) -> ()
    - getFinalDocuments() -> [TIMDocument]

Pipeline:

1. Go through all files in the project directory
    - For each file, check file extension
    - If valid file extension, register as a project file
    - Each project file has a default processor assigned to it and a method to get the front matter as JSON
    - Register each project file to the file processor
2. Generate the final document structure
    - Go through each processor and request to provide the final document structure
    - Each processor will return a list of final TIMDocument instances that contain information about path and title
3. Generate the files in TIM
    - For each TIMDocument, a document in TIM is generated
    - The ID is obtained and saved in the TIMDocument instance
4. Upload document contents
    - For each TIMDocument, request the document contents and upload them
    - Under the hood, the processor will generate the contents of the document
    