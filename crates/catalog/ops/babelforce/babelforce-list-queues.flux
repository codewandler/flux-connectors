op babelforce-list-queues(page: Number, max: Number) -> Any
  description "List queues"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/queues")
  response = http.request(method: "GET", query: { max, page }, url)
  return response
