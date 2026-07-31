op statuspage-incident-get(incident_id: String) -> Any
  description "Get one incident on this status page, including every update posted to it so far. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.statuspage.io/v1/pages/{page_id}"
  url = fmt("{base}/incidents/{incident_id}")
  response = http.request(method: "GET", url)
  return response
