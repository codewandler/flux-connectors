op babelforce-list-queue-triggers(queueId: String) -> Any
  description "List a queue's triggers"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/queues/{queueId}/triggers")
  response = http.request(method: "GET", url)
  return response
