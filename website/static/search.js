const searchRoot=document.querySelector("[data-search-root]");

if(searchRoot){
  const dropdown=searchRoot.querySelector("[data-search-dropdown]");
  const emptyMessage=searchRoot.querySelector("[data-search-empty]");
  const input=searchRoot.querySelector("[data-search-input]");
  const results=searchRoot.querySelector("[data-search-results]");
  const status=searchRoot.querySelector("[data-search-status]");
  const indexUrl=searchRoot.getAttribute("data-search-index-url");

  const normalize=(text)=>text.toLowerCase().trim();

  const renderResult=(entry,query)=>{
    const item=document.createElement("li");
    item.className="search-result";

    const link=document.createElement("a");
    link.className="search-result-link";
    link.href=entry.url;
    link.textContent=entry.title;

    const meta=document.createElement("div");
    meta.className="search-result-meta";
    meta.textContent=`${entry.section} · ${entry.url}`;

    const description=document.createElement("div");
    description.className="search-result-desc";
    description.textContent=entry.description.length>0?entry.description:entry.text.slice(0,180);

    item.appendChild(link);
    item.appendChild(meta);
    item.appendChild(description);

    if(query.length>0){
      item.dataset.query=query;
    }

    return item;
  };

  const setStatus=(message)=>{status.textContent=message;};

  const showDropdown=(visible)=>{dropdown.hidden=!visible;};

  const showEmpty=(visible)=>{emptyMessage.hidden=!visible;};

  const clearResults=()=>{results.replaceChildren();};

  const searchEntries=(entries,query)=>{
    if(query.length===0){
      return [];
    }

    return entries.filter((entry)=>{
      const haystack=normalize(`${entry.title}\n${entry.description}\n${entry.section}\n${entry.text}`);
      return haystack.includes(query);
    }).slice(0,20);
  };

  fetch(indexUrl).then((response)=>{
    if(!response.ok){
      throw new Error(`search index request failed: ${response.status}`);
    }

    return response.json();
  }).then((entries)=>{
    setStatus(`Loaded ${entries.length} indexed pages.`);

    input.addEventListener("input",()=>{
      const query=normalize(input.value);
      const matches=searchEntries(entries,query);

      clearResults();

      if(query.length===0){
        showDropdown(false);
        showEmpty(false);
        return;
      }

      showDropdown(true);
      showEmpty(matches.length===0);
      setStatus(matches.length===0?`No results for "${query}".`:`${matches.length} result${matches.length===1?"":"s"} for "${query}".`);
      matches.forEach((entry)=>results.appendChild(renderResult(entry,query)));
    });

    input.addEventListener("focus",()=>{
      if(normalize(input.value).length>0){
        showDropdown(true);
      }
    });

    document.addEventListener("click",(event)=>{
      if(!searchRoot.contains(event.target)){
        showDropdown(false);
      }
    });
  }).catch((error)=>{
    clearResults();
    showDropdown(true);
    showEmpty(true);
    setStatus(`Search is unavailable: ${error.message}`);
  });
}
