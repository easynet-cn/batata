package io.batata.tests;

import com.alibaba.nacos.api.config.ConfigChangeItem;
import com.alibaba.nacos.api.config.PropertyChangeType;
import com.alibaba.nacos.api.config.listener.ConfigChangeParser;

import java.io.IOException;
import java.util.HashMap;
import java.util.Map;

/**
 * ConfigChangeParser for text type configs.
 *
 * Treats the entire config content as a single property with key "content".
 * Aligned with Nacos test suite's TextChangeParser.
 */
public class TextChangeParser implements ConfigChangeParser {

    @Override
    public boolean isResponsibleFor(String type) {
        return (null == type || "text".equalsIgnoreCase(type));
    }

    @Override
    public Map<String, ConfigChangeItem> doParse(String oldContent, String newContent, String type) throws IOException {
        Map<String, ConfigChangeItem> map = new HashMap<>(4);
        final String key = "content";

        ConfigChangeItem cci = new ConfigChangeItem(key, oldContent, newContent);
        if (null == oldContent && null != newContent) {
            cci.setType(PropertyChangeType.ADDED);
        } else if (null != oldContent && null != newContent && !oldContent.equals(newContent)) {
            cci.setType(PropertyChangeType.MODIFIED);
        } else if (null != oldContent && null == newContent) {
            cci.setType(PropertyChangeType.DELETED);
        }
        map.put(key, cci);

        return map;
    }
}
