op babelforce-list-audit-logs(filters_dateCreated_start: Number, filters_dateCreated_end: Number, max: Number, page: Number, sort: String, order: String, filters_operation: String, filters_resource: String) -> Any
  description "Get a list of all audit logs"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/audit/request")
  response = http.request(method: "GET", query: { "filters.dateCreated.end": filters_dateCreated_end, "filters.dateCreated.start": filters_dateCreated_start, "filters.operation": filters_operation, "filters.resource": filters_resource, max, order, page, sort }, url)
  return response
