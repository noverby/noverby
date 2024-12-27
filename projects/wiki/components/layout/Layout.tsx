import React, { Suspense, startTransition, useEffect, useState } from 'react';
import {
  Scroll,
  BottomBar,
  OldBrowser,
  Drawer,
  MobileMenu,
  AppDrawer,
} from 'comps';
import { useAuthenticationStatus } from '@nhost/nextjs';
import { useRouter, useSearchParams } from 'next/navigation';
import { Container, Box, useMediaQuery, IconButton } from '@mui/material';
import { checkVersion } from 'core/util';
import { usePath, useSession } from 'hooks';
import { Refresh } from '@mui/icons-material';
import { useSnackbar } from 'notistack';

const Layout = ({ children }: { children: JSX.Element }) => {
  const [outdated, setOutdated] = useState(false);
  const [showing, setShowing] = useState(false);
  const [openDrawer, setOpenDrawer] = useState(false);
  const [session, setSession] = useSession();
  const router = useRouter();
  const params = useSearchParams();
  const path = usePath();
  const [version, setVersion] = useState<string | undefined>();
  const { isLoading } = useAuthenticationStatus();
  const largeScreen = useMediaQuery('(min-width:1200px)');
  const { enqueueSnackbar } = useSnackbar();

  useEffect(() => {
    startTransition(() => {
      setOutdated(typeof window !== 'undefined' && !checkVersion());
      setShowing(true);
    });
  }, []);

  useEffect(() => {
    if (session !== null && session?.timeDiff === undefined) {
      setSession({ timeDiff: 0 });
      fetch('/api/time').then((res) =>
        res.json().then(({ time }) => {
          setSession({
            timeDiff: new Date().getTime() - new Date(time).getTime(),
          });
        })
      );
    }
  }, [session, setSession]);

  useEffect(() => {
    const checkVersion = () => {
      fetch('/api/version').then((res) => {
        res.json().then(({ commit }) => {
          if (version == undefined) {
            setVersion(commit);
          } else if (version != commit) {
            enqueueSnackbar('Ny version tilgængelig', {
              variant: 'info',
              autoHideDuration: null,
              action: () => {
                return (
                  <IconButton onClick={() => startTransition(() => router.refresh() )}>
                    <Refresh />
                  </IconButton>
                );
              },
            });
          }
        });
      });
    };
    checkVersion();
    window.addEventListener('focus', checkVersion);
    return () => window.removeEventListener('focus', checkVersion);
  }, []);

  if (outdated) return <OldBrowser />;
  if (!showing || isLoading) return null;

  if (params.get("app") === 'screen' || path.startsWith('user'))
    return children;

  return (
      <Box sx={{ display: 'flex' }}>
        <Scroll>
          <>
            {largeScreen && <Box sx={{ p: 4 }} />}
            {typeof window !== 'undefined' && (
              <Container sx={{ pl: 1, pr: 1, pt: 1 }} disableGutters>
                {children}
              </Container>
            )}
            <BottomBar openDrawer={openDrawer} setOpenDrawer={setOpenDrawer} />
          </>
        </Scroll>
        {largeScreen && (
          <AppDrawer openDrawer={openDrawer} setOpenDrawer={setOpenDrawer} />
        )}

        <Drawer
          open={openDrawer}
          setOpen={() => startTransition(() => setOpenDrawer(false))}
        />

        {!largeScreen && (
          <Suspense>
            <MobileMenu openDrawer={openDrawer} setOpenDrawer={setOpenDrawer} />
          </Suspense>
        )}
      </Box>
  );
};

export default Layout;
