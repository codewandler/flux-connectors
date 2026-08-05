op statuspage-incident-list -> Any
  description "List this status page's incidents, most recent first — both unresolved and already-resolved ones. Returns Statuspage's own first page of results; this connector declares no paging parameters, so a page with a long incident history is not enumerated exhaustively here. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.statuspage.io/v1/pages/{page_id}"
  url = fmt("{base}/incidents")
  response = http.request(method: "GET", url)
  return response
