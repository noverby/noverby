import platform from 'platform';
import { compare } from 'compare-versions';

const checkVersion = () => {
  switch (platform.layout) {
    case 'Gecko':
      if (compare(platform.version ?? '0', '94', '<=')) return false;
    case 'Blink':
      if (
        compare(platform.version ?? '0', '98', '<=') &&
        platform.name !== 'Opera'
      )
        return false;
    case 'WebKit':
      if (compare(platform.version ?? '0', '15.4', '<=')) return false;
    default:
      return true;
  }
};

export { checkVersion };
