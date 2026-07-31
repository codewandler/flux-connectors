op sendgrid-suppression-bounce-list(start_time: Number, end_time: Number) -> Any
  description "List addresses that have bounced and are suppressed from receiving further mail. Each entry is personal data about a third party — read it only for what the calling flow needs"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.sendgrid.com"
  url = fmt("{base}/v3/suppression/bounces")
  sep = "?"
  when start_time
    url = fmt("{url}{sep}start_time={start_time}")
    sep = "&"
  when end_time
    url = fmt("{url}{sep}end_time={end_time}")
  response = http.request(method: "GET", url)
  return response
