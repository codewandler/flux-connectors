op babelforce-list-audit-logs(filters_dateCreated_start: Number, filters_dateCreated_end: Number, max: Number, page: Number, sort: String, order: String, filters_operation: String, filters_resource: String) -> Any
  description "Get a list of all audit logs"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/audit/request")
  sep = "?"
  when filters_dateCreated_start
    url = fmt("{url}{sep}filters.dateCreated.start={filters_dateCreated_start}")
    sep = "&"
  when filters_dateCreated_end
    url = fmt("{url}{sep}filters.dateCreated.end={filters_dateCreated_end}")
    sep = "&"
  when max
    url = fmt("{url}{sep}max={max}")
    sep = "&"
  when page
    url = fmt("{url}{sep}page={page}")
    sep = "&"
  when sort
    url = fmt("{url}{sep}sort={sort}")
    sep = "&"
  when order
    url = fmt("{url}{sep}order={order}")
    sep = "&"
  when filters_operation
    url = fmt("{url}{sep}filters.operation={filters_operation}")
    sep = "&"
  when filters_resource
    url = fmt("{url}{sep}filters.resource={filters_resource}")
  response = http.request(method: "GET", url)
  return response
