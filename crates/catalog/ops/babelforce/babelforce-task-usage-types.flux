op babelforce-task-usage-types(start: String, end: String) -> Any
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v3/tasks/usage/types")
  sep = "?"
  when start
    url = fmt("{url}{sep}start={start}")
    sep = "&"
  when end
    url = fmt("{url}{sep}end={end}")
  response = http.request(method: "GET", url)
  return response
