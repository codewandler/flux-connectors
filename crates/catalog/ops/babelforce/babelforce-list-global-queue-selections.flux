op babelforce-list-global-queue-selections(sort: String, order: String, includeMembers: Bool, page: Number, max: Number) -> Any
  description "List all queue selections"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/queues/selections")
  response = http.request(method: "GET", query: { includeMembers, max, order, page, sort }, url)
  return response
