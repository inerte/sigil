function isWordLikeReplacement(text) {
  return /^[A-Za-z_][A-Za-z0-9_]*$/u.test(text);
}

function isWordLikeChar(char) {
  return !!char && /[A-Za-z0-9_]/u.test(char);
}

function shouldInsertLeadingSeparator(source, startOffset, replacement) {
  if (!isWordLikeReplacement(replacement)) {
    return false;
  }

  const prevChar = source[startOffset - 1] || '';
  return isWordLikeChar(prevChar);
}

function shouldInsertTrailingSeparator(source, endOffset, replacement) {
  if (!isWordLikeReplacement(replacement)) {
    return false;
  }

  const nextChar = source[endOffset] || '';
  return isWordLikeChar(nextChar);
}

function rewriteSingleToken(source, token, replacement) {
  const startIndex = token.start.index;
  const endIndex = token.end.index;
  const prefix = source.slice(0, startIndex);
  const suffix = source.slice(endIndex);
  const leadingSeparator = shouldInsertLeadingSeparator(source, startIndex, replacement) ? ' ' : '';
  const trailingSeparator = shouldInsertTrailingSeparator(source, endIndex, replacement) ? ' ' : '';

  return `${prefix}${leadingSeparator}${replacement}${trailingSeparator}${suffix}`;
}

export function rewriteFileForTokenType(file, targetTokenType, replacement) {
  if (!file.symbols.some((token) => token.type === targetTokenType)) {
    return file.source;
  }

  let cursor = 0;
  let output = '';

  for (const token of file.tokens) {
    output += file.source.slice(cursor, token.start.index);

    if (token.type === targetTokenType) {
      const leadingSeparator = shouldInsertLeadingSeparator(file.source, token.start.index, replacement) ? ' ' : '';
      const trailingSeparator = shouldInsertTrailingSeparator(file.source, token.end.index, replacement) ? ' ' : '';
      output += `${leadingSeparator}${replacement}${trailingSeparator}`;
    } else {
      output += file.source.slice(token.start.index, token.end.index);
    }

    cursor = token.end.index;
  }

  output += file.source.slice(cursor);
  return output;
}

export function buildBoundarySnippet(file, token, replacement, radius = 16) {
  const start = Math.max(0, token.start.index - radius);
  const end = Math.min(file.source.length, token.end.index + radius);
  const snippet = file.source.slice(start, end);
  const shiftedToken = {
    ...token,
    start: {
      ...token.start,
      index: token.start.index - start
    },
    end: {
      ...token.end,
      index: token.end.index - start
    }
  };

  return {
    before: snippet,
    after: rewriteSingleToken(snippet, shiftedToken, replacement)
  };
}
