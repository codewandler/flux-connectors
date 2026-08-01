op babelforce-list-global-queue-selections(sort: String, order: String, includeMembers: Bool, page: Number, max: Number) -> Any
  description "List all queue selections"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/queues/selections")
  sep = "?"
  when sort
    url = fmt("{url}{sep}sort={sort}")
    sep = "&"
  when order
    url = fmt("{url}{sep}order={order}")
    sep = "&"
  when includeMembers
    url = fmt("{url}{sep}includeMembers={includeMembers}")
    sep = "&"
  when page
    url = fmt("{url}{sep}page={page}")
    sep = "&"
  when max
    url = fmt("{url}{sep}max={max}")
  response = http.request(method: "GET", url)
  return response
