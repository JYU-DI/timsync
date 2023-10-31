# TIMSync - CLI tool to sync local documents with TIM

TIMSync is a tool to sync local documents with TIM.

The tool is aimed to allow authoring and managing documents easily outside of TIM.
The features will include (checked items are implemented):

- [x] Managing multiple sync targets (local TIM, hosted TIM, different folders on the same TIM instance, etc.)
- [x] Syncing documents to TIM automatically
- [ ] Local templating preprocessor to allow splitting document and managing them in a more modular way
- [ ] Automatic management of course attachments (images, videos, etc.) and links

See the [issue tracker](https://github.com/JYU-DI/timsync/issues?q=is%3Aissue+is%3Aopen+label%3Afeature) for more
planned features.

## Quick start

To get started:

0. Compile the CLI tool using latest stable Rust (**TODO**: add precompiled binaries)
1. **Create a separate TIM user for the CLI tool.**

   Currently, the tool currently stores authentication in the project configuration.
   Therefore, create a separate TIM account by going to TIM homepage and selecting **Log in** -> **Sign up**.

2. Create a folder in TIM to which the files will be synced. Add owner permissions to the user created in step 1.

3. Create a new project in the current folder using

    ```bash
     timsync init
    ```

   Provide the required information (host, TIM username, TIM password, path to the folder created in step 2).

   After this, you should see a `.timsync` folder with an appropriate `.gitignore` file created in the folder.

4. Create and edit markdown files.

   **NOTE:** Currently only basic editing is supported. Link and attachment management is not yet supported.

5. Sync the files to TIM using

    ```bash
    timsync sync
    ```

   This will upload the files to TIM.

   **Note:** Folders and Markdown files that start with a dot `.` or an underscore `_` are not uploaded.
   These files and folders are reserved for future templating and attachment management features.

## Command description

Run `timsync help` to get help with the command line options.

Current usage is:

```
$ timsync --help
A tool to preprocess and synchronize documents to TIM

TIMSync is a preprocessor and synchronizer for TIM documents. It allows to upload documents and files to TIM.

Usage: timsync <COMMAND>

Commands:
  init
          Initialize a new TIMSync project
  sync
          Synchronize the project with TIM
  help
          Print this message or the help of the given subcommand(s)

Options:
  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```