op contentful-entries-list(limit: Number, skip: Number) -> Any
  description "List published entries in this space and environment, most recently updated first. This connector's `verify` — a bounded read that runs unattended: space and environment are already resolved from configuration, and `limit`/`skip` are optional, so no required argument is ever needed"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://cdn.contentful.com/spaces/{space_id}/environments/{environment_id}"
  url = fmt("{base}/entries")
  sep = "?"
  when limit
    url = fmt("{url}{sep}limit={limit}")
    sep = "&"
  when skip
    url = fmt("{url}{sep}skip={skip}")
  response = http.request(method: "GET", url)
  return response
