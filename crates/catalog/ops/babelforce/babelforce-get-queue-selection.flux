op babelforce-get-queue-selection(queueId: String, id: String) -> Any
  description "Get a queue selection"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/queues/{queueId}/selections/{id}")
  response = http.request(method: "GET", url)
  return response
