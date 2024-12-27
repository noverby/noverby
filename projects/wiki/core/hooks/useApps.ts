import { useSearchParams } from 'next/navigation';
import { useLink, usePath, useSession } from 'hooks';
import { useAuthenticated, useUserEmail, useUserId } from '@nhost/nextjs';
import { useSubscription } from 'gql';
import { useMediaQuery } from '@mui/material';

const useApps = () => {
  const pathname = usePath();
  const params = useSearchParams();
  const link = useLink();
  const [session] = useSession();
  const isAuthenticated = useAuthenticated();
  const userId = useUserId();
  const email = useUserEmail();
  const sub = useSubscription();
  const largeScreen = useMediaQuery('(min-width:1200px)');

  const currentApp = params.get('app') ?? (pathname == '' ? 'home' : 'folder');

  const handleClick = (path?: string[], app?: string) => async () => {
    const scroll = document.querySelector('#scroll');
    localStorage.setItem(
      `scroll/${currentApp}`,
      scroll?.scrollTop?.toString() ?? '0'
    );
    localStorage.setItem('app', currentApp);
    if (
      (localStorage['app'] ?? 'folder') === 'folder' &&
      app == undefined &&
      path?.length != 0
    ) {
      return;
    }
    if (currentApp === 'folder') {
      localStorage.setItem('path', pathname);
    }
    await link.path(path ?? [], app);
    scroll?.scrollTo(
      0,
      JSON.parse(localStorage[`scroll/${app ?? 'folder'}`] ?? 0)
    );
  };

  return [
    {
      name: 'Hjem',
      mimeId: 'app/home',
      active: ['home'].includes(currentApp),
      onClick: handleClick([]),
      notifications: isAuthenticated
        ? sub
            .membersAggregate({
              where: {
                _and: [
                  { accepted: { _eq: false } },
                  {
                    _or: [
                      { nodeId: { _eq: userId } },
                      { email: { _eq: email } },
                    ],
                  },

                  { parent: { mimeId: { _in: ['wiki/group', 'wiki/event'] } } },
                ],
              },
            })
            .aggregate?.count()
        : 0,
    },
    ...(currentApp !== 'home'
      ? [
          {
            name: 'Mappe',
            mimeId: 'app/folder',
            active: ['folder', 'editor'].includes(currentApp),
            onClick: handleClick(
              localStorage?.path?.split('/') ?? session?.prefix?.path
            ),
            notifications: 0,
          },
        ]
      : []),
    ...(isAuthenticated && currentApp !== 'home'
      ? [
          {
            name: 'Tal',
            mimeId: 'app/speak',
            active: ['speak'].includes(currentApp),
            onClick: handleClick(session?.prefix?.path, 'speak'),
            notifications: 0,
          },
          {
            name: 'Stem',
            mimeId: 'app/vote',
            active: ['vote', 'poll'].includes(currentApp),
            onClick: handleClick(session?.prefix?.path, 'vote'),
            notifications: 0,
          },
        ]
      : currentApp !== 'home' && largeScreen ? [{ name: "Skærm", mimeId: "app/screen", active: [], onClick: async () =>  {
          window.open(`/${session?.prefix?.path?.join('/')}?app=screen`, '_blank')

      }, notification: 0 }] : []),
  ];
};

export default useApps;
