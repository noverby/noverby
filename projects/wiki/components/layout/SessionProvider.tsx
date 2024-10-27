import React, { useState } from 'react';
import { Session, SessionContext } from 'hooks';

const SessionProvider = ({ children }: { children: JSX.Element }) => {
  const [session, setSession] = useState<Session | null>(null);

  return (
    <SessionContext.Provider value={[session, setSession]}>
      {children}
    </SessionContext.Provider>
  );
};

export default SessionProvider;
