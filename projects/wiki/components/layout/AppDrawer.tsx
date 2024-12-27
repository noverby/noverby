import { AppList } from 'comps';
import { Suspense } from 'react';

const AppDrawer = ({
  openDrawer,
  setOpenDrawer,
}: {
  openDrawer: boolean;
  setOpenDrawer: (val: boolean) => void;
}) => {
  return (
      <Suspense>
        <AppList />
      </Suspense>
  );
};

export default AppDrawer;
