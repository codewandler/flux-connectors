op notion-page-create(parent_page_id: String, title: List<Any>) -> Any
  description "Create a new empty page as a child of an existing page. The page is created with a title and no content — this connector cannot write page body text, which in Notion is a separate tree of blocks. The parent page must be shared with this integration. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/message`, its error code at `/code` in the response body."
  risk "medium"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://api.notion.com"
  url = fmt("{base}/v1/pages")
  content_type = "application/json"
  Notion_Version = "2022-06-28"
  payload = { parent: { page_id: parent_page_id }, properties: { title } }
  response = http.request(body: payload, headers: { "Notion-Version": Notion_Version, "content-type": content_type }, method: "POST", url)
  return response
