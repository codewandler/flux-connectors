op babelforce-get-agents-for-queue-selection(queueId: String, callId: String) -> Any
  description "Preview a selection's agents"
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/queues/{queueId}/select")
  sep = "?"
  when callId
    url = fmt("{url}{sep}callId={callId}")
  response = http.request(method: "POST", url)
  return response
