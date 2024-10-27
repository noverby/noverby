import { useEffect, createContext, useContext, startTransition } from 'react';

export type Session = {
  path?: string;
  prefix?: { id?: string; name: string; mime: string; path: string[] };
  user?: { id: string; displayName: string; email: string };
  timeDiff?: number;
  theme?: string;
  screen?: { size: string };
  selected?: string[];
  nodeId?: string | null;
};
type SessionSetter = (props: Session | null) => void;

export const SessionContext = createContext<
  [Session | null, SessionSetter] | null
>(null);

const useSession = (): [Session | null, SessionSetter] => {
  const [session, setCtxSession] = useContext(SessionContext) || [null, null];

  useEffect(() => {
    if (!session && setCtxSession && localStorage.session) {
      startTransition(() => {
        setCtxSession(JSON.parse(localStorage.session));
      });
    }
  }, [session]);

  const setSession: SessionSetter = (props) => {
    const newSession =
      !session || props === null ? props : { ...session, ...props };
    localStorage.setItem('session', JSON.stringify(newSession));
    if (setCtxSession) setCtxSession(newSession);
  };

  return [session, setSession];
};

export default useSession;
