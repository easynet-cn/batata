const DEFAULT_SERVER_PORT: i32 = 8848;
const SERVER_PORT_PROPERTY: &str = "server.port";
const SPRING_MANAGEMENT_CONTEXT_NAMESPACE: &str = "management";
const MEMBER_CHANGE_EVENT_QUEUE_SIZE_PROPERTY: &str = "nacos.member-change-event.queue.size";
const DEFAULT_MEMBER_CHANGE_EVENT_QUEUE_SIZE: i32 = 128;
const DEFAULT_TASK_DELAY_TIME: i64 = 5000;

static isUseAddressServer: bool = false;
