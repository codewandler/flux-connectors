op notion-page-get(page_id: String) -> Any
  description "Get one page's properties, parent, icon, cover and timestamps. This does NOT return the page's text — in Notion a page's content is a separate tree of blocks, which this connector cannot read. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/message`, its error code at `/code` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.notion.com"
  url = fmt("{base}/v1/pages/{page_id}")
  Notion_Version = "2022-06-28"
  response = http.request(headers: { "Notion-Version": Notion_Version }, method: "GET", url)
  return response
