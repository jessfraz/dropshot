// Copyright 2020 Oxide Computer Company
/*!
 * Example showing use of the "Simple" pagination API
 *
 * When you run this program, it will start an HTTP server on an available local
 * port.  See the log entry to see what port it ran on.  Then use curl to use
 * it, like this:
 *
 * ```ignore
 * $ curl localhost:50568/projects
 * ```
 *
 * (Replace 50568 with whatever port your server is listening on.)
 *
 * Try passing different values of the `limit` query parameter.  Try passing the
 * next page token from the response as a query parameter, too.
 */

use dropshot::endpoint;
use dropshot::ApiDescription;
use dropshot::ConfigDropshot;
use dropshot::ConfigLogging;
use dropshot::ConfigLoggingLevel;
use dropshot::HttpError;
use dropshot::HttpResponseOkPage;
use dropshot::HttpServer;
use dropshot::PaginationParams;
use dropshot::Query;
use dropshot::RequestContext;
use dropshot::SimplePageSelector;
use dropshot::SimplePaginated;
use dropshot::WhichPage;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::ops::RangeFrom;
use std::sync::Arc;

/**
 * Structure on which we hang our implementation of [`SimplePaginated`].
 */
// XXX shouldn't need to be Deserialize
#[derive(Deserialize)]
struct ProjectScan;
impl SimplePaginated for ProjectScan {
    type PaginatedField = String;
    type SimpleItem = Project;

    fn page_selector_for_item(p: &Project) -> String {
        p.name.clone()
    }
}

/**
 * Object returned by our paginated endpoint
 *
 * Like anything returned by Dropshot, we must implement `JsonSchema` and
 * `Serialize`.  We also implement `Clone` to simplify the example.
 */
#[derive(Clone, JsonSchema, Serialize)]
struct Project {
    name: String,
    // lots more fields
}

/**
 * API endpoint for listing projects
 *
 * This implementation stores all the projects in a BTreeMap, which makes it
 * very easy to fetch a particular range of items based on the key.
 */
#[endpoint {
    method = GET,
    path = "/projects"
}]
async fn example_list_projects(
    rqctx: Arc<RequestContext>,
    query: Query<PaginationParams<ProjectScan>>,
) -> Result<HttpResponseOkPage<ProjectScan>, HttpError> {
    let pag_params = query.into_inner();
    let limit = rqctx.page_limit(&pag_params)?.get();
    let tree = rqctx_to_tree(rqctx);
    let projects = match &pag_params.page_params {
        WhichPage::FirstPage {
            ..
        } => {
            /* Return a list of the first "limit" projects. */
            tree.iter()
                .take(limit)
                .map(|(_, project)| project.clone())
                .collect()
        }
        WhichPage::NextPage {
            page_token,
        } => {
            /* Return a list of the first "limit" projects after this name. */
            let SimplePageSelector::FullScan(last_seen) =
                &page_token.page_start;
            tree.range(RangeFrom {
                start: last_seen.clone(),
            })
            .take(limit)
            .map(|(_, project)| project.clone())
            .collect()
        }
    };

    Ok(HttpResponseOkPage(ProjectScan::scan_mode(), projects))
}

fn rqctx_to_tree(rqctx: Arc<RequestContext>) -> Arc<BTreeMap<String, Project>> {
    let c = Arc::clone(&rqctx.server.private);
    c.downcast::<BTreeMap<String, Project>>().unwrap()
}

#[tokio::main]
async fn main() -> Result<(), String> {
    /*
     * Create 1000 projects up front.
     */
    let mut tree = BTreeMap::new();
    for n in 1..1000 {
        let name = format!("project{:03}", n);
        let project = Project {
            name: name.clone(),
        };
        tree.insert(name, project);
    }

    /*
     * Run the Dropshot server.
     */
    let ctx = Arc::new(tree);
    let config_dropshot = ConfigDropshot {
        bind_address: "127.0.0.1:0".parse().unwrap(),
    };
    let config_logging = ConfigLogging::StderrTerminal {
        level: ConfigLoggingLevel::Debug,
    };
    let log = config_logging
        .to_logger("example-pagination-basic")
        .map_err(|error| format!("failed to create logger: {}", error))?;
    let mut api = ApiDescription::new();
    api.register(example_list_projects).unwrap();
    let mut server = HttpServer::new(&config_dropshot, api, ctx, &log)
        .map_err(|error| format!("failed to create server: {}", error))?;
    let server_task = server.run();
    server.wait_for_shutdown(server_task).await
}