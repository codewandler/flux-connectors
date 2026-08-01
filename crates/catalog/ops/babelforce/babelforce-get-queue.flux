op babelforce-get-queue(id: String) -> Any
  description "Get a queue"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/queues/{id}")
  response = http.request(method: "GET", url)
  return response
