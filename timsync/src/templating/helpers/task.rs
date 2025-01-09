use crate::processing::task_processor::{TASKS_REF_MAP_KEY, TASKS_UID};
use crate::templating::util::get_site_ctx_json;
use crate::util::tim_client::hashed_par_id;
use handlebars::{
    Context, Handlebars, Helper, HelperResult, Output, RenderContext, RenderErrorReason,
};

/// Task helper.
/// Inserts a reference to a specific task plugin based on the task UID.
///
/// **Note**: The helper requires that there is at least one task (`*.task.yml` file) in the project.
///
/// Example:
///
/// `task-example.task.yml`:
///
/// ```yaml
/// ---
/// uid: task1
/// plugin: csPlugin
/// ---
/// type: text
/// header: Task Test
/// rows: 10
/// ```
///
/// `task-example.md`:
///
/// ```md
/// Task 1:
///
/// {{task "task1"}}
/// ```
pub fn task_helper<'reg, 'rc>(
    h: &Helper<'rc>,
    _: &'reg Handlebars<'reg>,
    ctx: &'rc Context,
    _: &mut RenderContext<'reg, 'rc>,
    out: &mut dyn Output,
) -> HelperResult {
    let task_id = h
        .param(0)
        .ok_or_else(|| RenderErrorReason::ParamNotFoundForIndex("task_id", 0))?
        .value()
        .as_str()
        .ok_or_else(|| {
            RenderErrorReason::ParamTypeMismatchForName(
                "task_id",
                "0".to_string(),
                "string".to_string(),
            )
        })?;

    let site_ctx_json = get_site_ctx_json(ctx)?;

    let task_ref_map = site_ctx_json.get(TASKS_REF_MAP_KEY).ok_or_else(|| {
        RenderErrorReason::Other("There are no tasks registered in the project. Add tasks (`.task.yml` files) to the project to use the task helper.".to_string())
    })?.as_object().expect("Task reference map is not an object");
    let doc_map = site_ctx_json
        .get("doc")
        .expect("Document map is not set")
        .as_object()
        .expect("Document map is not an object");

    let task_doc_id = doc_map
        .get(TASKS_UID)
        .map(|v| v.as_object().expect("Task document is not an object"))
        .map(|v| {
            v.get("doc_id")
                .expect("Task document ID is not set")
                .as_u64()
                .expect("Task document ID is not a number")
        })
        .expect("Task document is missing from the document list.");

    let task_par_id = task_ref_map.get(task_id).map(|v| v.as_str().expect("Par ID is not a string")).ok_or_else(|| {
        RenderErrorReason::Other(format!("Task with UID '{}' is not registered in the project. Check that the UID is written correctly.", task_id))
    })?;

    let par_id = hashed_par_id(Some(task_id));

    out.write(&format!(
        "#- {{ rd=\"{}\" rp=\"{}\" id=\"{}\" }}\n#-\n",
        task_doc_id, task_par_id, par_id
    ))?;

    Ok(())
}
