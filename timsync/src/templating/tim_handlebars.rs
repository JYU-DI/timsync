use crate::project::project::Project;
use crate::templating::helpers::area::area_block;
use crate::templating::helpers::docsettings::docsettings_block;
use crate::templating::helpers::file::file_helper;
use crate::templating::helpers::gen_par_id::gen_par_id_helper;
use crate::templating::helpers::include::include_helper;
use crate::templating::helpers::ref_area::ref_area_helper;
use crate::templating::helpers::task::task_helper;
use crate::templating::helpers::task_id::task_id_helper;
use crate::templating::helpers::url_for::url_for_helper;
use anyhow::Context;
use handlebars::Handlebars;

pub const FILE_MAP_ATTRIBUTE: &str = "$_timsync_upload_files";
const TEMPLATE_FOLDER: &str = "_templates";
const HELPERS_FOLDER: &str = "_helpers";

pub trait TimRendererExt
where
    Self: Sized,
{
    /// Extend the renderer instance with the TIM templates for documents.
    ///
    /// returns: &Self
    fn with_tim_doc_helpers(self) -> Self;

    /// Extend the renderer instance with the file helpers.
    ///
    /// returns: &Self
    fn with_base_helpers(self) -> Self;

    /// Extend the renderer instance with the project templates.
    /// The templates may be used as partials in the rendering process.
    ///
    /// Templates are scanned from the `_templates` folder in a project.
    /// All files in the folder are registered as templates.
    ///
    /// # Arguments
    ///
    /// * `project`: The project to get the templates from.
    ///
    /// returns: Result<Self, Error>
    fn with_project_templates(self, project: &Project) -> anyhow::Result<Self>;

    /// Extend the renderer instance with the project helpers.
    /// The helpers are used to extend the templating engine with custom scripts.
    ///
    /// Helpers are scanned from the `_helpers` folder in a project.
    /// The helpers must be written in the Rhai scripting language (file extension `.rhai`).
    ///
    /// # Arguments
    ///
    /// * `project`: The project to get the helpers from.
    ///
    /// returns: Result<Self, Error>
    fn with_project_helpers(self, project: &Project) -> anyhow::Result<Self>;
}

impl TimRendererExt for Handlebars<'_> {
    fn with_tim_doc_helpers(mut self) -> Self {
        self.register_escape_fn(handlebars::no_escape);
        self.register_helper("area", Box::new(area_block));
        self.register_helper("docsettings", Box::new(docsettings_block));
        self.register_helper("ref_area", Box::new(ref_area_helper));
        self.register_helper("task", Box::new(task_helper));
        handlebars_misc_helpers::register(&mut self);
        self.with_base_helpers()
    }

    fn with_base_helpers(mut self) -> Self {
        self.register_helper("include", Box::new(include_helper));
        self.register_helper("file", Box::new(file_helper));
        self.register_helper("task_id", Box::new(task_id_helper));
        self.register_helper("url_for", Box::new(url_for_helper));
        self.register_helper("gen_par_id", Box::new(gen_par_id_helper));
        self
    }

    fn with_project_templates(mut self, project: &Project) -> anyhow::Result<Self> {
        let template_files = project
            .find_files(TEMPLATE_FOLDER, "*")
            .with_context(|| format!("Could not find templates from folder {}", TEMPLATE_FOLDER))?;
        for (name, template) in template_files {
            self.register_template_file(&name, template)?;
        }

        Ok(self)
    }

    fn with_project_helpers(mut self, project: &Project) -> anyhow::Result<Self> {
        let helper_files = project
            .find_files(HELPERS_FOLDER, "*.rhai")
            .with_context(|| format!("Could not find helpers from folder {}", HELPERS_FOLDER))?;
        for (name, helper) in helper_files {
            let name = name.trim_end_matches(".rhai");
            self.register_script_helper_file(&name, helper)?;
        }

        Ok(self)
    }
}
