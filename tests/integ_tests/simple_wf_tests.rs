use assert_matches::assert_matches;
use rand::{self, Rng};
use std::{convert::TryFrom, env, time::Duration};
use temporal_sdk_core::{
    protos::{
        coresdk::{wf_activation_job, TaskCompletion, TimerFiredTaskAttributes, WfActivationJob},
        temporal::api::command::v1::{
            CompleteWorkflowExecutionCommandAttributes, StartTimerCommandAttributes,
        },
    },
    Core, CoreInitOptions, ServerGatewayOptions, Url,
};

// TODO: These tests can get broken permanently if they break one time and the server is not
//  restarted, because pulling from the same task queue produces tasks for the previous failed
//  workflows. Fix that.

const NAMESPACE: &str = "default";

#[tokio::main]
async fn create_workflow(core: &dyn Core, task_q: &str, workflow_id: &str) -> String {
    core.server_gateway()
        .unwrap()
        .start_workflow(NAMESPACE, task_q, workflow_id, "test-workflow")
        .await
        .unwrap()
        .run_id
}

#[test]
fn timer_workflow() {
    let task_q = "timer_workflow";
    let temporal_server_address = match env::var("TEMPORAL_SERVICE_ADDRESS") {
        Ok(addr) => addr,
        Err(_) => "http://localhost:7233".to_owned(),
    };
    let url = Url::try_from(&*temporal_server_address).unwrap();
    let gateway_opts = ServerGatewayOptions {
        namespace: NAMESPACE.to_string(),
        identity: "none".to_string(),
        worker_binary_id: "".to_string(),
        long_poll_timeout: Duration::from_secs(60),
        target_url: url,
    };
    let core = temporal_sdk_core::init(CoreInitOptions { gateway_opts }).unwrap();
    let mut rng = rand::thread_rng();
    let workflow_id: u32 = rng.gen();
    dbg!(create_workflow(&core, task_q, &workflow_id.to_string()));
    let timer_id: String = rng.gen::<u32>().to_string();
    let task = core.poll_task(task_q).unwrap();
    core.complete_task(TaskCompletion::ok_from_api_attrs(
        vec![StartTimerCommandAttributes {
            timer_id: timer_id.to_string(),
            start_to_fire_timeout: Some(Duration::from_secs(1).into()),
            ..Default::default()
        }
        .into()],
        task.task_token,
    ))
    .unwrap();
    let task = dbg!(core.poll_task(task_q).unwrap());
    core.complete_task(TaskCompletion::ok_from_api_attrs(
        vec![CompleteWorkflowExecutionCommandAttributes { result: None }.into()],
        task.task_token,
    ))
    .unwrap();
}

#[test]
fn parallel_timer_workflow() {
    let task_q = "parallel_timer_workflow";
    let temporal_server_address = match env::var("TEMPORAL_SERVICE_ADDRESS") {
        Ok(addr) => addr,
        Err(_) => "http://localhost:7233".to_owned(),
    };
    let url = Url::try_from(&*temporal_server_address).unwrap();
    let gateway_opts = ServerGatewayOptions {
        namespace: NAMESPACE.to_string(),
        identity: "none".to_string(),
        worker_binary_id: "".to_string(),
        long_poll_timeout: Duration::from_secs(60),
        target_url: url,
    };
    let core = temporal_sdk_core::init(CoreInitOptions { gateway_opts }).unwrap();
    let mut rng = rand::thread_rng();
    let workflow_id: u32 = rng.gen();
    dbg!(create_workflow(&core, task_q, &workflow_id.to_string()));
    let timer_id = "timer 1".to_string();
    let timer_2_id = "timer 2".to_string();
    let task = dbg!(core.poll_task(task_q).unwrap());
    core.complete_task(TaskCompletion::ok_from_api_attrs(
        vec![
            StartTimerCommandAttributes {
                timer_id: timer_id.clone(),
                start_to_fire_timeout: Some(Duration::from_millis(50).into()),
                ..Default::default()
            }
            .into(),
            StartTimerCommandAttributes {
                timer_id: timer_2_id.clone(),
                start_to_fire_timeout: Some(Duration::from_millis(100).into()),
                ..Default::default()
            }
            .into(),
        ],
        task.task_token,
    ))
    .unwrap();
    // Wait long enough for both timers to complete
    std::thread::sleep(Duration::from_millis(1000));
    let task = core.poll_task(task_q).unwrap();
    assert_matches!(
        task.get_wf_jobs().as_slice(),
        [
            WfActivationJob {
                attributes: Some(wf_activation_job::Attributes::TimerFired(
                    TimerFiredTaskAttributes { timer_id: t1_id }
                )),
            },
            WfActivationJob {
                attributes: Some(wf_activation_job::Attributes::TimerFired(
                    TimerFiredTaskAttributes { timer_id: t2_id }
                )),
            }
        ] => {
            assert_eq!(t1_id, &timer_id);
            assert_eq!(t2_id, &timer_2_id);
        }
    );
    core.complete_task(TaskCompletion::ok_from_api_attrs(
        vec![CompleteWorkflowExecutionCommandAttributes { result: None }.into()],
        task.task_token,
    ))
    .unwrap();
}
