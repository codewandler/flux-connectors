op babelforce-list-queued-calls(queueId: String, page: Number, max: Number) -> Any
  description "List a queue's waiting calls"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/queues/{queueId}/calls")
  response = http.request(method: "GET", query: { max, page }, url)
  return response
