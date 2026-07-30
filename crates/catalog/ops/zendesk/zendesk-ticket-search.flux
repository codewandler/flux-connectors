op zendesk-ticket-search(query: String, page: Number, per_page: Number) -> Any
  description "Search tickets with Zendesk search syntax, e.g. `type:ticket status:new`"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  $base = "https://{subdomain}.zendesk.com"
  $url = fmt("{base}/api/v2/search.json?query={query}")
  $sep = "&"
  when $page
    $url = fmt("{url}{sep}page={page}")
    $sep = "&"
  when $per_page
    $url = fmt("{url}{sep}per_page={per_page}")
  $response = http.request({ method: "GET", url: $url })
  return $response
