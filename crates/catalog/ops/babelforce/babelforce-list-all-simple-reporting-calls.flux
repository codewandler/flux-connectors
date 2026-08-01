op babelforce-list-all-simple-reporting-calls(page: Number, max: Number, sessionId: String, conversationId: String, id: Any, parentId: Any, type: Any, from: String, fromNumber: String, to: Any, toNumber: Any, time_start: Number, time_end: Number, agentId: Any, queueName: String, agentName: String, bridged: Bool, duration_start: Number, duration_end: Number, waitTime_start: Number, waitTime_end: Number, queueWaitTime_start: Number, queueWaitTime_end: Number, bridgeTime_start: Number, bridgeTime_end: Number, talkTime_start: Number, talkTime_end: Number, holdTime_start: Number, holdTime_end: Number, wrapupTime_start: Number, wrapupTime_end: Number, handleTime_start: Number, handleTime_end: Number) -> Any
  description "List reporting calls (simple)"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/calls/reporting/simple")
  sep = "?"
  when page
    url = fmt("{url}{sep}page={page}")
    sep = "&"
  when max
    url = fmt("{url}{sep}max={max}")
    sep = "&"
  when sessionId
    url = fmt("{url}{sep}sessionId={sessionId}")
    sep = "&"
  when conversationId
    url = fmt("{url}{sep}conversationId={conversationId}")
    sep = "&"
  when id
    url = fmt("{url}{sep}id={id}")
    sep = "&"
  when parentId
    url = fmt("{url}{sep}parentId={parentId}")
    sep = "&"
  when type
    url = fmt("{url}{sep}type={type}")
    sep = "&"
  when from
    url = fmt("{url}{sep}from={from}")
    sep = "&"
  when fromNumber
    url = fmt("{url}{sep}fromNumber={fromNumber}")
    sep = "&"
  when to
    url = fmt("{url}{sep}to={to}")
    sep = "&"
  when toNumber
    url = fmt("{url}{sep}toNumber={toNumber}")
    sep = "&"
  when time_start
    url = fmt("{url}{sep}time.start={time_start}")
    sep = "&"
  when time_end
    url = fmt("{url}{sep}time.end={time_end}")
    sep = "&"
  when agentId
    url = fmt("{url}{sep}agentId={agentId}")
    sep = "&"
  when queueName
    url = fmt("{url}{sep}queueName={queueName}")
    sep = "&"
  when agentName
    url = fmt("{url}{sep}agentName={agentName}")
    sep = "&"
  when bridged
    url = fmt("{url}{sep}bridged={bridged}")
    sep = "&"
  when duration_start
    url = fmt("{url}{sep}duration.start={duration_start}")
    sep = "&"
  when duration_end
    url = fmt("{url}{sep}duration.end={duration_end}")
    sep = "&"
  when waitTime_start
    url = fmt("{url}{sep}waitTime.start={waitTime_start}")
    sep = "&"
  when waitTime_end
    url = fmt("{url}{sep}waitTime.end={waitTime_end}")
    sep = "&"
  when queueWaitTime_start
    url = fmt("{url}{sep}queueWaitTime.start={queueWaitTime_start}")
    sep = "&"
  when queueWaitTime_end
    url = fmt("{url}{sep}queueWaitTime.end={queueWaitTime_end}")
    sep = "&"
  when bridgeTime_start
    url = fmt("{url}{sep}bridgeTime.start={bridgeTime_start}")
    sep = "&"
  when bridgeTime_end
    url = fmt("{url}{sep}bridgeTime.end={bridgeTime_end}")
    sep = "&"
  when talkTime_start
    url = fmt("{url}{sep}talkTime.start={talkTime_start}")
    sep = "&"
  when talkTime_end
    url = fmt("{url}{sep}talkTime.end={talkTime_end}")
    sep = "&"
  when holdTime_start
    url = fmt("{url}{sep}holdTime.start={holdTime_start}")
    sep = "&"
  when holdTime_end
    url = fmt("{url}{sep}holdTime.end={holdTime_end}")
    sep = "&"
  when wrapupTime_start
    url = fmt("{url}{sep}wrapupTime.start={wrapupTime_start}")
    sep = "&"
  when wrapupTime_end
    url = fmt("{url}{sep}wrapupTime.end={wrapupTime_end}")
    sep = "&"
  when handleTime_start
    url = fmt("{url}{sep}handleTime.start={handleTime_start}")
    sep = "&"
  when handleTime_end
    url = fmt("{url}{sep}handleTime.end={handleTime_end}")
  response = http.request(method: "GET", url)
  return response
