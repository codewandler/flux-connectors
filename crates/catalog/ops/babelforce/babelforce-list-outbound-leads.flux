op babelforce-list-outbound-leads(page: Number, max: Number, status: String, listId: String) -> Any
  description "List outbound leads"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/outbound/leads")
  sep = "?"
  when page
    url = fmt("{url}{sep}page={page}")
    sep = "&"
  when max
    url = fmt("{url}{sep}max={max}")
    sep = "&"
  when status
    url = fmt("{url}{sep}status={status}")
    sep = "&"
  when listId
    url = fmt("{url}{sep}listId={listId}")
  response = http.request(method: "GET", url)
  return response
