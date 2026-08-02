op zendesk-incremental-organization-list(start_time: Number, per_page: Number) -> Any
  description "Incrementally export organizations updated at or after a required Unix start time with an optional integer page size"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{subdomain}.zendesk.com"
  url = fmt("{base}/api/v2/incremental/organizations?start_time={start_time}")
  sep = "&"
  when per_page
    url = fmt("{url}{sep}per_page={per_page}")
  response = http.request(method: "GET", url)
  return response
