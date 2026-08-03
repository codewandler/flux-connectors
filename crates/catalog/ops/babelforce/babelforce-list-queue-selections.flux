op babelforce-list-queue-selections(queueId: String, page: Number, max: Number) -> Any
  description "List a queue's selections"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/queues/{queueId}/selections")
  response = http.request(method: "GET", query: { max, page }, url)
  return response
