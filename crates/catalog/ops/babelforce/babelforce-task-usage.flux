op babelforce-task-usage(type: String, start: String, end: String, rate: String) -> Any
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v3/tasks/usage")
  sep = "?"
  when type
    url = fmt("{url}{sep}type={type}")
    sep = "&"
  when start
    url = fmt("{url}{sep}start={start}")
    sep = "&"
  when end
    url = fmt("{url}{sep}end={end}")
    sep = "&"
  when rate
    url = fmt("{url}{sep}rate={rate}")
  response = http.request(method: "GET", url)
  return response
