op newrelic-application-list -> Any
  description "List every application this account is monitoring with APM, each with its current health status and a summary of its response time, throughput and error rate. Takes no argument. Also this connector's `verify`: a bounded read that runs unattended and needs nothing beyond the configured host and key"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{host}/v2"
  url = fmt("{base}/applications.json")
  response = http.request(method: "GET", url)
  return response
