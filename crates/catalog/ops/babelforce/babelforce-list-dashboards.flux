op babelforce-list-dashboards(page: Number, max: Number, q: String, uuid: String, sort: String, order: String) -> Any
  description "List dashboards"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/dashboards")
  response = http.request(method: "GET", query: { max, order, page, q, sort, uuid }, url)
  return response
