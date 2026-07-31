op statuspage-component-list -> Any
  description "List the components this status page publishes — the individual services whose operational state the page shows. Takes no parameter at all and succeeds for any page the API key can administer, which is what makes it the connection check for a settings page's Test Connection button. Returns Statuspage's own first page of results; this connector declares no paging parameters. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.statuspage.io/v1/pages/{page_id}"
  url = fmt("{base}/components")
  response = http.request(method: "GET", url)
  return response
