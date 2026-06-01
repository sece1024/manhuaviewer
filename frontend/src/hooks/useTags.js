import React, { useState, useEffect, useCallback, useContext, createContext } from 'react';
import api from '../utils/api';

const TagsContext = createContext(null);

export function TagsProvider({ children }) {
  const [tags, setTags] = useState([]);
  const [loaded, setLoaded] = useState(false);

  const reload = useCallback(() => {
    return api.getTags().then(data => {
      setTags(data);
      setLoaded(true);
      return data;
    }).catch(() => {
      setLoaded(true);
      return [];
    });
  }, []);

  useEffect(() => {
    reload();
  }, [reload]);

  return (
    <TagsContext.Provider value={{ tags, loaded, reload }}>
      {children}
    </TagsContext.Provider>
  );
}

export default function useTags() {
  const ctx = useContext(TagsContext);
  if (!ctx) throw new Error('useTags must be used within TagsProvider');
  return ctx;
}
