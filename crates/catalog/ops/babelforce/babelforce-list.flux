op babelforce-list(filter: String, type: String, details: String, page_size: Number, page: Number, context: Bool) -> Any
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v3/tasks/logs/customer")
  sep = "?"
  when filter
    url = fmt("{url}{sep}filter={filter}")
    sep = "&"
  when type
    url = fmt("{url}{sep}type={type}")
    sep = "&"
  when details
    url = fmt("{url}{sep}details={details}")
    sep = "&"
  when page_size
    url = fmt("{url}{sep}page_size={page_size}")
    sep = "&"
  when page
    url = fmt("{url}{sep}page={page}")
    sep = "&"
  when context
    url = fmt("{url}{sep}context={context}")
  response = http.request(method: "GET", url)
  return response
