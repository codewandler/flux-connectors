op babelforce-get-agents-for-queue-selection(queueId: String, callId: String) -> Any
  description "Preview a selection's agents"
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/queues/{queueId}/select")
  response = http.request(method: "POST", query: { callId }, url)
  return response
