op newrelic-application-get(application_id: String) -> Any
  description "Get one application by id, with its current health status and the same summary figures the list returns. Use this to re-read one application after a deploy rather than listing the whole account"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{host}/v2"
  url = fmt("{base}/applications/{application_id}.json")
  response = http.request(method: "GET", url)
  return response
